//! Read-only skill discovery for planning.
//!
//! Searches the 1Agent skill journal and user-installed `SKILL.md` roots.
//! It returns metadata and the skill body so the planner can decide whether a
//! Worker should apply the skill; it never activates, evaluates or runs one.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::time::Duration;

use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use tokio_util::sync::CancellationToken;

use super::{AgentTool, ToolError, ToolOutcome};
use crate::domain::{PermissionSet, RiskLevel, ToolSpec};
use crate::skill::SkillManifest;
use crate::skill_service::SkillService;

const MAX_RESULTS: usize = 10;
const MAX_BODY_CHARS: usize = 4_000;
const MAX_DESCRIPTION_CHARS: usize = 300;
const MAX_FILES: usize = 500;
const MAX_DEPTH: usize = 6;

pub struct SkillSearchTool {
    roots: Vec<PathBuf>,
    store: Option<PathBuf>,
}

impl SkillSearchTool {
    pub fn new(roots: Vec<PathBuf>, store: Option<PathBuf>) -> Self {
        Self { roots, store }
    }

    pub fn from_env() -> Self {
        let roots = std::env::var_os("ONEAGENT_SKILL_ROOTS")
            .map(|value| {
                std::env::split_paths(&value)
                    .filter(|path| path.is_dir())
                    .collect::<Vec<_>>()
            })
            .unwrap_or_else(default_roots);
        let store = std::env::var_os("ONEAGENT_SKILL_JOURNAL")
            .map(PathBuf::from)
            .unwrap_or_else(|| crate::runtime_home::default_home().join("skills.jsonl"));
        Self::new(roots, store.is_file().then_some(store))
    }
}

fn default_roots() -> Vec<PathBuf> {
    let Some(home) = std::env::var_os("HOME").map(PathBuf::from) else {
        return Vec::new();
    };
    [
        home.join(".claude/skills"),
        home.join(".claude/plugins/marketplaces"),
        home.join(".1a/skills"),
    ]
    .into_iter()
    .filter(|path| path.is_dir())
    .collect()
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct SkillSearchArgs {
    #[serde(default)]
    query: String,
    #[serde(default = "default_max_results")]
    max_results: usize,
    #[serde(default = "default_include_body")]
    include_body: bool,
}

fn default_max_results() -> usize {
    5
}

fn default_include_body() -> bool {
    true
}

#[derive(Debug, Clone, Serialize)]
struct SkillHit {
    name: String,
    description: String,
    version: String,
    status: String,
    source: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    path: Option<String>,
    requires_tools: Vec<String>,
    risk: String,
    body: String,
    body_truncated: bool,
}

#[async_trait]
impl AgentTool for SkillSearchTool {
    fn spec(&self) -> ToolSpec {
        ToolSpec {
            name: "skill_search".into(),
            description: "Search installed skills (the 1Agent skill journal and user SKILL.md roots) by name, description or body. Returns skill metadata, path and a bounded body so the planner can assign the skill to a Worker. Read-only; it never activates or executes a skill.".into(),
            parameters: json!({
                "type": "object",
                "additionalProperties": false,
                "properties": {
                    "query": {"type": "string", "default": ""},
                    "max_results": {"type": "integer", "minimum": 1, "maximum": MAX_RESULTS, "default": 5},
                    "include_body": {"type": "boolean", "default": true}
                }
            }),
        }
    }

    fn risk(&self) -> RiskLevel {
        RiskLevel::Low
    }

    fn parallel_safe(&self) -> bool {
        true
    }

    fn timeout(&self) -> Duration {
        Duration::from_secs(15)
    }

    async fn execute(
        &self,
        arguments: Value,
        _permissions: &PermissionSet,
        _cancellation: &CancellationToken,
    ) -> Result<ToolOutcome, ToolError> {
        let args: SkillSearchArgs = serde_json::from_value(arguments)
            .map_err(|error| ToolError::InvalidArguments(error.to_string()))?;
        if args.max_results == 0 || args.max_results > MAX_RESULTS {
            return Err(ToolError::InvalidArguments(format!(
                "max_results must be between 1 and {MAX_RESULTS}"
            )));
        }
        let query = args.query.trim().to_ascii_lowercase();
        let mut hits = BTreeMap::<String, SkillHit>::new();

        if let Some(store) = &self.store
            && let Ok(service) = SkillService::open(store.clone())
        {
            for summary in service.list() {
                if !matches(&query, &summary.name, &summary.description, "") {
                    continue;
                }
                let Some(manifest) = service.view(&summary.name, &summary.version) else {
                    continue;
                };
                let hit = manifest_hit(manifest, summary.status.as_str(), args.include_body);
                hits.entry(hit.name.clone()).or_insert(hit);
            }
        }

        let mut files = 0usize;
        for root in &self.roots {
            scan_root(root, 0, &mut files, &query, args.include_body, &mut hits);
            if files >= MAX_FILES {
                break;
            }
        }

        let mut values = hits.into_values().collect::<Vec<_>>();
        values.sort_by(|left, right| {
            score(right, &query)
                .cmp(&score(left, &query))
                .then_with(|| left.name.cmp(&right.name))
        });
        let total = values.len();
        values.truncate(args.max_results);
        let content = serde_json::to_string_pretty(&json!({
            "query": args.query,
            "skills": values,
            "matched": total,
            "truncated": total > args.max_results,
        }))
        .map_err(|error| ToolError::Failed(format!("cannot serialize skill search: {error}")))?;
        Ok(ToolOutcome {
            summary: format!("found {total} skill(s)"),
            content,
            metadata: BTreeMap::new(),
        })
    }
}

fn manifest_hit(manifest: &SkillManifest, status: &str, include_body: bool) -> SkillHit {
    let (body, body_truncated) = bounded_body(&manifest.body, include_body);
    SkillHit {
        name: manifest.name.clone(),
        description: bounded(&manifest.description, MAX_DESCRIPTION_CHARS),
        version: manifest.version.clone(),
        status: status.into(),
        source: "journalled".into(),
        path: None,
        requires_tools: manifest.requires_tools.clone(),
        risk: format!("{:?}", manifest.risk).to_ascii_lowercase(),
        body,
        body_truncated,
    }
}

fn scan_root(
    dir: &Path,
    depth: usize,
    files: &mut usize,
    query: &str,
    include_body: bool,
    hits: &mut BTreeMap<String, SkillHit>,
) {
    if depth > MAX_DEPTH || *files >= MAX_FILES {
        return;
    }
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    let mut entries = entries.flatten().collect::<Vec<_>>();
    entries.sort_by_key(|entry| entry.file_name());
    for entry in entries {
        if *files >= MAX_FILES {
            return;
        }
        let path = entry.path();
        let Ok(metadata) = std::fs::symlink_metadata(&path) else {
            continue;
        };
        if metadata.file_type().is_symlink() {
            continue;
        }
        let name = entry.file_name();
        let name = name.to_string_lossy();
        if metadata.is_dir() {
            if matches!(
                name.as_ref(),
                ".git" | "node_modules" | "target" | ".cache" | "dist"
            ) {
                continue;
            }
            scan_root(&path, depth + 1, files, query, include_body, hits);
            continue;
        }
        if name != "SKILL.md" {
            continue;
        }
        *files += 1;
        let Ok(source) = std::fs::read_to_string(&path) else {
            continue;
        };
        if source.len() > 256 * 1024 {
            continue;
        }
        let Some(skill) = parse_skill_markdown(&source) else {
            continue;
        };
        if !matches(query, &skill.name, &skill.description, &skill.body) {
            continue;
        }
        let (body, body_truncated) = bounded_body(&skill.body, include_body);
        hits.entry(skill.name.clone()).or_insert(SkillHit {
            name: skill.name,
            description: bounded(&skill.description, MAX_DESCRIPTION_CHARS),
            version: skill.version,
            status: "external".into(),
            source: "filesystem".into(),
            path: Some(path.display().to_string()),
            requires_tools: skill.requires_tools,
            risk: skill.risk,
            body,
            body_truncated,
        });
    }
}

struct ParsedSkill {
    name: String,
    description: String,
    version: String,
    requires_tools: Vec<String>,
    risk: String,
    body: String,
}

fn parse_skill_markdown(source: &str) -> Option<ParsedSkill> {
    let mut lines = source.lines();
    if lines.next()?.trim() != "---" {
        return None;
    }
    let mut front = BTreeMap::<String, String>::new();
    let mut last_key: Option<String> = None;
    let mut body = String::new();
    let mut in_frontmatter = true;
    for line in lines {
        if in_frontmatter {
            if line.trim() == "---" {
                in_frontmatter = false;
                continue;
            }
            if line.starts_with(' ') || line.starts_with('\t') {
                if let Some(key) = &last_key {
                    let value = front.entry(key.clone()).or_default();
                    if !value.is_empty() {
                        value.push(' ');
                    }
                    value.push_str(line.trim());
                }
                continue;
            }
            let Some((key, value)) = line.split_once(':') else {
                continue;
            };
            let key = key.trim().to_ascii_lowercase();
            let value = value.trim();
            if value == ">" || value == "|" || value.is_empty() {
                front.entry(key.clone()).or_default();
                last_key = Some(key);
            } else {
                front.insert(key, value.trim_matches('"').to_string());
                last_key = None;
            }
        } else {
            body.push_str(line);
            body.push('\n');
        }
    }
    let name = front.get("name")?.trim().to_string();
    if name.is_empty() {
        return None;
    }
    let description = front
        .get("description")
        .cloned()
        .unwrap_or_default()
        .trim()
        .to_string();
    let version = front.get("version").cloned().unwrap_or_else(|| "0".into());
    let requires_tools = front
        .get("requires_tools")
        .or_else(|| front.get("allowed-tools"))
        .or_else(|| front.get("allowed_tools"))
        .map(|value| parse_list(value))
        .unwrap_or_default();
    let risk = front.get("risk").cloned().unwrap_or_else(|| "low".into());
    Some(ParsedSkill {
        name,
        description,
        version,
        requires_tools,
        risk,
        body: body.trim().to_string(),
    })
}

fn parse_list(value: &str) -> Vec<String> {
    value
        .trim()
        .trim_start_matches('[')
        .trim_end_matches(']')
        .split(',')
        .map(|item| item.trim().trim_matches('"').trim_matches('\''))
        .filter(|item| !item.is_empty())
        .map(str::to_string)
        .collect()
}

fn matches(query: &str, name: &str, description: &str, body: &str) -> bool {
    if query.is_empty() {
        return true;
    }
    let haystack = format!("{name}\n{description}\n{body}").to_ascii_lowercase();
    haystack.contains(query)
}

fn score(hit: &SkillHit, query: &str) -> usize {
    if query.is_empty() {
        return 0;
    }
    let name = hit.name.to_ascii_lowercase();
    let description = hit.description.to_ascii_lowercase();
    let body = hit.body.to_ascii_lowercase();
    let mut score = 0;
    if name.contains(query) {
        score += 3;
    }
    if description.contains(query) {
        score += 2;
    }
    if body.contains(query) {
        score += 1;
    }
    score
}

fn bounded(value: &str, max_chars: usize) -> String {
    let mut bounded: String = value.chars().take(max_chars).collect();
    if value.chars().nth(max_chars).is_some() {
        bounded.push('…');
    }
    bounded
}

fn bounded_body(body: &str, include: bool) -> (String, bool) {
    if !include {
        return (String::new(), false);
    }
    let truncated = body.chars().count() > MAX_BODY_CHARS;
    (bounded(body, MAX_BODY_CHARS), truncated)
}

#[cfg(all(test, unix))]
mod tests {
    use super::*;

    fn write_skill(root: &Path, dir: &str, source: &str) {
        let path = root.join(dir);
        std::fs::create_dir_all(&path).unwrap();
        std::fs::write(path.join("SKILL.md"), source).unwrap();
    }

    #[tokio::test]
    async fn finds_filesystem_skills_and_returns_the_body() {
        let dir = tempfile::tempdir().unwrap();
        write_skill(
            dir.path(),
            "lark-doc",
            "---\nname: lark-doc\ndescription: Read Lark documents with lark-cli\nallowed-tools: [shell, lark-cli]\n---\n# Lark\nUse `lark-cli docs +fetch`.\n",
        );
        write_skill(
            dir.path(),
            "release",
            "---\nname: release\ndescription: Release checklist\n---\n# Release\n",
        );
        let tool = SkillSearchTool::new(vec![dir.path().to_path_buf()], None);
        let outcome = tool
            .execute(
                json!({"query": "lark"}),
                &PermissionSet::default(),
                &CancellationToken::new(),
            )
            .await
            .unwrap();
        assert!(outcome.content.contains("lark-doc"));
        assert!(outcome.content.contains("lark-cli docs +fetch"));
        assert!(outcome.content.contains("shell"));
        assert!(!outcome.content.contains("# Release"));
    }

    #[tokio::test]
    async fn returns_empty_for_no_match() {
        let dir = tempfile::tempdir().unwrap();
        write_skill(
            dir.path(),
            "release",
            "---\nname: release\ndescription: Release checklist\n---\nbody\n",
        );
        let tool = SkillSearchTool::new(vec![dir.path().to_path_buf()], None);
        let outcome = tool
            .execute(
                json!({"query": "nonexistent"}),
                &PermissionSet::default(),
                &CancellationToken::new(),
            )
            .await
            .unwrap();
        assert!(outcome.content.contains("\"skills\": []"));
    }

    #[tokio::test]
    async fn finds_journalled_skills() {
        let dir = tempfile::tempdir().unwrap();
        let store = dir.path().join("skills.jsonl");
        let mut service = SkillService::open(store.clone()).unwrap();
        service
            .register(
                SkillManifest::from_markdown(
                    "---\nname: release\ndescription: release process\nversion: 1.0.0\nrequires_tools: [shell]\nrisk: medium\n---\n# Release\nrun tests",
                )
                .unwrap(),
                1,
            )
            .unwrap();
        let tool = SkillSearchTool::new(Vec::new(), Some(store));
        let outcome = tool
            .execute(
                json!({"query": "release"}),
                &PermissionSet::default(),
                &CancellationToken::new(),
            )
            .await
            .unwrap();
        assert!(outcome.content.contains("release"));
        assert!(outcome.content.contains("run tests"));
        assert!(outcome.content.contains("journalled"));
    }
}
