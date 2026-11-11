//! Deterministic tool search over name, description and parameter names.
//!
//! This is a pure ranking function. It does not change the tool registry or
//! what the model may call; it answers "which tools are relevant to this
//! query?" so a later slice can load definitions on demand.

use serde::{Deserialize, Serialize};

use crate::domain::ToolSpec;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct ToolSearchHit {
    pub spec: ToolSpec,
    pub score: u32,
}

pub fn search(query: &str, specs: &[ToolSpec], limit: usize) -> Vec<ToolSearchHit> {
    let query_tokens = tokens(query);
    if query_tokens.is_empty() || limit == 0 {
        return Vec::new();
    }
    let mut hits = specs
        .iter()
        .filter_map(|spec| {
            let name = spec.name.to_ascii_lowercase();
            let name_tokens = tokens(&spec.name.replace('_', " "));
            let description_tokens = tokens(&spec.description);
            let parameter_tokens = tokens(&spec.parameters.to_string());
            let mut score = 0u32;
            for token in &query_tokens {
                if name == *token {
                    score = score.saturating_add(100);
                } else if name.starts_with(&format!("{token}_")) {
                    score = score.saturating_add(30);
                } else if name_tokens.contains(token) {
                    score = score.saturating_add(20);
                }
                if description_tokens.contains(token) {
                    score = score.saturating_add(5);
                }
                if parameter_tokens.contains(token) {
                    score = score.saturating_add(2);
                }
            }
            (score > 0).then(|| ToolSearchHit {
                spec: spec.clone(),
                score,
            })
        })
        .collect::<Vec<_>>();
    hits.sort_by(|left, right| {
        right
            .score
            .cmp(&left.score)
            .then_with(|| left.spec.name.cmp(&right.spec.name))
    });
    hits.truncate(limit);
    hits
}

fn tokens(text: &str) -> Vec<String> {
    text.split(|ch: char| !ch.is_alphanumeric())
        .filter(|token| !token.is_empty())
        .map(|token| token.to_ascii_lowercase())
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn spec(name: &str, description: &str) -> ToolSpec {
        ToolSpec {
            name: name.into(),
            description: description.into(),
            parameters: json!({"type": "object", "properties": {"path": {"type": "string"}}}),
        }
    }

    fn specs() -> Vec<ToolSpec> {
        vec![
            spec("read_file", "Read a file from the workspace"),
            spec("write_file", "Write a file to the workspace"),
            spec("browser_read", "Read a public web page"),
            spec("terminal_exec", "Run a shell command"),
        ]
    }

    #[test]
    fn exact_name_wins_and_read_matches_both_readers() {
        let hits = search("read", &specs(), 10);
        let names = hits
            .iter()
            .map(|hit| hit.spec.name.as_str())
            .collect::<Vec<_>>();
        assert_eq!(names[0], "read_file");
        assert!(names.contains(&"browser_read"));
        assert!(!names.contains(&"write_file"));
    }

    #[test]
    fn query_is_case_insensitive_and_uses_description() {
        let hits = search("SHELL", &specs(), 10);
        assert_eq!(hits[0].spec.name, "terminal_exec");
    }

    #[test]
    fn empty_query_and_zero_limit_return_nothing() {
        assert!(search("", &specs(), 10).is_empty());
        assert!(search("read", &specs(), 0).is_empty());
        assert!(search("zzz", &specs(), 10).is_empty());
    }

    #[test]
    fn limit_and_deterministic_ties_are_respected() {
        let hits = search("file", &specs(), 1);
        assert_eq!(hits.len(), 1);
        assert_eq!(hits[0].spec.name, "read_file");
    }
}
