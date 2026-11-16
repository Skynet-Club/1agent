//! Capability-first tool discovery.
//!
//! The catalog is data (`capabilities/catalog.json`), not a task-type enum.
//! The search tool maps a capability phrase to candidate tools/skills/libraries,
//! probe steps, verification recipes and fallbacks. It never installs or runs a
//! candidate; the Agent must choose, adapt and verify.

use std::collections::BTreeMap;
use std::sync::OnceLock;
use std::time::Duration;

use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

use super::{AgentTool, ToolError, ToolOutcome};
use crate::domain::{PermissionSet, RiskLevel, ToolSpec};

const CATALOG_JSON: &str = include_str!("../../capabilities/catalog.json");
const DEFAULT_LIMIT: usize = 5;
const MAX_LIMIT: usize = 10;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct CapabilityCatalog {
    pub version: u32,
    pub capabilities: Vec<CapabilityCard>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct CapabilityCard {
    pub id: String,
    pub name: String,
    pub summary: String,
    #[serde(default)]
    pub keywords: Vec<String>,
    pub inputs: Vec<String>,
    pub operation: String,
    pub outputs: Vec<String>,
    #[serde(default)]
    pub constraints: Vec<String>,
    #[serde(default)]
    pub probe: Vec<String>,
    pub candidates: Vec<CapabilityCandidate>,
    pub verification: Vec<String>,
    #[serde(default)]
    pub fallback: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct CapabilityCandidate {
    pub name: String,
    pub kind: String,
    pub source: String,
    pub license: String,
    pub trust: String,
    #[serde(default)]
    pub notes: String,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct CapabilityMatch {
    pub score: u32,
    #[serde(flatten)]
    pub capability: CapabilityCard,
}

fn catalog() -> Result<&'static CapabilityCatalog, String> {
    static CATALOG: OnceLock<Result<CapabilityCatalog, String>> = OnceLock::new();
    CATALOG
        .get_or_init(|| {
            let parsed: CapabilityCatalog = serde_json::from_str(CATALOG_JSON)
                .map_err(|error| format!("invalid capability catalog: {error}"))?;
            validate_catalog(&parsed)?;
            Ok(parsed)
        })
        .as_ref()
        .map_err(Clone::clone)
}

pub fn validate_catalog(catalog: &CapabilityCatalog) -> Result<(), String> {
    if catalog.version != 1 {
        return Err(format!(
            "unsupported capability catalog version {}",
            catalog.version
        ));
    }
    if catalog.capabilities.is_empty() || catalog.capabilities.len() > 100 {
        return Err("capability catalog must contain 1..=100 capabilities".into());
    }
    let mut ids = std::collections::BTreeSet::new();
    for capability in &catalog.capabilities {
        for (field, value) in [
            ("id", &capability.id),
            ("name", &capability.name),
            ("summary", &capability.summary),
            ("operation", &capability.operation),
        ] {
            if value.trim().is_empty() {
                return Err(format!("capability {} has an empty {field}", capability.id));
            }
        }
        if !ids.insert(capability.id.as_str()) {
            return Err(format!("duplicate capability id {}", capability.id));
        }
        if capability.inputs.is_empty()
            || capability.outputs.is_empty()
            || capability.candidates.is_empty()
            || capability.verification.is_empty()
        {
            return Err(format!(
                "capability {} needs inputs, outputs, candidates and verification",
                capability.id
            ));
        }
        let mut candidate_names = std::collections::BTreeSet::new();
        for candidate in &capability.candidates {
            if candidate.name.trim().is_empty()
                || candidate.kind.trim().is_empty()
                || candidate.source.trim().is_empty()
                || candidate.license.trim().is_empty()
            {
                return Err(format!(
                    "capability {} has an incomplete candidate",
                    capability.id
                ));
            }
            if !matches!(
                candidate.kind.as_str(),
                "tool" | "skill" | "cli" | "library" | "service" | "build" | "human"
            ) {
                return Err(format!(
                    "capability {} has unknown candidate kind {}",
                    capability.id, candidate.kind
                ));
            }
            if !matches!(
                candidate.trust.as_str(),
                "core" | "verified" | "community" | "experimental"
            ) {
                return Err(format!(
                    "capability {} has unknown trust level {}",
                    capability.id, candidate.trust
                ));
            }
            if !candidate_names.insert(candidate.name.as_str()) {
                return Err(format!(
                    "capability {} repeats candidate {}",
                    capability.id, candidate.name
                ));
            }
        }
    }
    Ok(())
}

pub fn search_capabilities(query: &str, limit: usize) -> Result<Vec<CapabilityMatch>, String> {
    let catalog = catalog()?;
    let query = query.trim();
    if query.is_empty() {
        return Err("capability query must not be empty".into());
    }
    let limit = limit.clamp(1, MAX_LIMIT);
    let normalized_query = normalize(query);
    let tokens = tokens(query);
    let mut matches = catalog
        .capabilities
        .iter()
        .filter_map(|capability| {
            let score = score(capability, &normalized_query, &tokens);
            (score > 0).then(|| CapabilityMatch {
                score,
                capability: capability.clone(),
            })
        })
        .collect::<Vec<_>>();
    matches.sort_by(|left, right| {
        right
            .score
            .cmp(&left.score)
            .then_with(|| left.capability.id.cmp(&right.capability.id))
    });
    matches.truncate(limit);
    Ok(matches)
}

fn normalize(text: &str) -> String {
    text.chars()
        .filter(|character| !character.is_whitespace() && !character.is_ascii_punctuation())
        .flat_map(char::to_lowercase)
        .collect()
}

fn tokens(text: &str) -> Vec<String> {
    text.split(|character: char| !character.is_alphanumeric())
        .filter(|token| !token.is_empty())
        .map(|token| token.to_lowercase())
        .collect()
}

fn score(capability: &CapabilityCard, normalized_query: &str, tokens: &[String]) -> u32 {
    let id = capability.id.to_lowercase();
    let name = capability.name.to_lowercase();
    let name_normalized = normalize(&capability.name);
    let mut score = 0u32;
    if id == normalized_query || name_normalized == normalized_query {
        score += 120;
    } else if id.contains(normalized_query) || name_normalized.contains(normalized_query) {
        score += 60;
    }

    for keyword in &capability.keywords {
        let keyword = keyword.to_lowercase();
        let keyword_normalized = normalize(&keyword);
        if keyword_normalized == normalized_query {
            score += 100;
        } else if !keyword_normalized.is_empty()
            && (normalized_query.contains(&keyword_normalized)
                || keyword_normalized.contains(normalized_query))
        {
            score += 45;
        }
    }

    let summary = capability.summary.to_lowercase();
    let summary_normalized = normalize(&summary);
    if summary_normalized.contains(normalized_query) {
        score += 30;
    }
    let mut haystack = String::new();
    for value in capability
        .inputs
        .iter()
        .chain(capability.outputs.iter())
        .chain(capability.constraints.iter())
    {
        haystack.push_str(&value.to_lowercase());
        haystack.push(' ');
    }
    haystack.push_str(&capability.operation.to_lowercase());
    haystack.push(' ');
    for candidate in &capability.candidates {
        haystack.push_str(&candidate.name.to_lowercase());
        haystack.push(' ');
        haystack.push_str(&candidate.notes.to_lowercase());
        haystack.push(' ');
    }
    let haystack_normalized = normalize(&haystack);
    if haystack_normalized.contains(normalized_query) {
        score += 20;
    }
    for token in tokens {
        if name.contains(token) {
            score += 8;
        }
        if haystack.contains(token) {
            score += 3;
        }
        if capability
            .keywords
            .iter()
            .any(|keyword| keyword.to_lowercase().contains(token))
        {
            score += 6;
        }
    }
    score
}

pub struct CapabilitySearchTool;

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct CapabilitySearchArgs {
    query: String,
    #[serde(default)]
    limit: Option<usize>,
}

#[async_trait]
impl AgentTool for CapabilitySearchTool {
    fn spec(&self) -> ToolSpec {
        ToolSpec {
            name: "capability_search".into(),
            description: "Search the local capability catalog by task capability (inputs, operation, output, constraints). Returns matching capability cards with candidate tools/skills, probe steps, verification recipes and fallbacks. Read-only; it never installs or runs a candidate. Use before a non-trivial build or conversion.".into(),
            parameters: json!({
                "type": "object",
                "additionalProperties": false,
                "required": ["query"],
                "properties": {
                    "query": {"type": "string"},
                    "limit": {"type": "integer", "minimum": 1, "maximum": 10, "default": 5}
                }
            }),
        }
    }

    fn risk(&self) -> RiskLevel {
        RiskLevel::Low
    }

    fn timeout(&self) -> Duration {
        Duration::from_secs(5)
    }

    async fn execute(
        &self,
        arguments: Value,
        _permissions: &PermissionSet,
        _cancellation: &tokio_util::sync::CancellationToken,
    ) -> Result<ToolOutcome, ToolError> {
        let args: CapabilitySearchArgs = serde_json::from_value(arguments)
            .map_err(|error| ToolError::InvalidArguments(error.to_string()))?;
        let query = args.query.trim().to_string();
        if query.is_empty() {
            return Err(ToolError::InvalidArguments(
                "query must not be empty".into(),
            ));
        }
        let limit = args.limit.unwrap_or(DEFAULT_LIMIT);
        let matches = search_capabilities(&query, limit).map_err(ToolError::Failed)?;
        let note = if matches.is_empty() {
            "no local capability matched; probe the input, search allowed external sources, or ask one constraint question before building".to_string()
        } else {
            "choose by fit, license, security, maintenance, integration cost and reversibility; probe before prescribing; a tool running is not verification".to_string()
        };
        let content = json!({
            "query": query,
            "matches": matches,
            "note": note
        })
        .to_string();
        let mut metadata = BTreeMap::new();
        metadata.insert("matches".into(), matches.len().to_string());
        metadata.insert("catalog_version".into(), "1".into());
        Ok(ToolOutcome {
            summary: format!("capability search: {} match(es)", matches.len()),
            content,
            metadata,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn catalog_is_valid_and_has_the_six_initial_cards() {
        let catalog = catalog().expect("catalog");
        assert_eq!(catalog.capabilities.len(), 6);
        let ids = catalog
            .capabilities
            .iter()
            .map(|capability| capability.id.as_str())
            .collect::<std::collections::BTreeSet<_>>();
        for id in [
            "design.ui_ux_page",
            "design.ui_review",
            "ocr.image_to_text",
            "pdf.to_html",
            "audio.transcribe",
            "video.edit",
        ] {
            assert!(ids.contains(id), "missing {id}");
        }
    }

    #[test]
    fn search_finds_ocr_for_scanned_pdf_text() {
        let matches = search_capabilities("扫描 PDF 提取文字", 5).expect("search");
        assert_eq!(matches[0].capability.id, "ocr.image_to_text");
    }

    #[test]
    fn search_finds_unslop_review() {
        let matches = search_capabilities("unslop 页面 AI 味 验收", 5).expect("search");
        assert_eq!(matches[0].capability.id, "design.ui_review");
    }

    #[test]
    fn search_finds_pdf_conversion() {
        let matches = search_capabilities("PDF 转 HTML", 5).expect("search");
        assert_eq!(matches[0].capability.id, "pdf.to_html");
    }

    #[test]
    fn search_finds_audio_transcription() {
        let matches = search_capabilities("音频转文字", 5).expect("search");
        assert_eq!(matches[0].capability.id, "audio.transcribe");
    }

    #[test]
    fn search_finds_video_editing() {
        let matches = search_capabilities("视频剪辑 加字幕", 5).expect("search");
        assert_eq!(matches[0].capability.id, "video.edit");
    }

    #[test]
    fn search_finds_design_for_a_landing_page() {
        let matches = search_capabilities("做一个产品落地页设计", 5).expect("search");
        assert_eq!(matches[0].capability.id, "design.ui_ux_page");
    }

    #[test]
    fn search_is_deterministic_and_bounded() {
        let first = search_capabilities("PDF", 2).expect("first");
        let second = search_capabilities("PDF", 2).expect("second");
        assert_eq!(first, second);
        assert!(first.len() <= 2);
    }

    #[test]
    fn empty_query_fails() {
        assert!(search_capabilities("", 5).is_err());
    }
}
