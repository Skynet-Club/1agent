//! Golden-set prompt inventory for M19–M23 answer-quality review.
//!
//! This module validates the prompt set only. It does not run a model, collect
//! labels or claim a blind-review result.

use std::collections::{BTreeMap, BTreeSet};

use serde::{Deserialize, Serialize};

pub const MIN_GOLDEN_PROMPTS: usize = 30;
pub const MAX_GOLDEN_PROMPTS: usize = 50;

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord)]
#[serde(rename_all = "snake_case")]
pub enum GoldenCategory {
    Chat,
    Fact,
    Decision,
    Research,
    Execution,
}

impl GoldenCategory {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Chat => "chat",
            Self::Fact => "fact",
            Self::Decision => "decision",
            Self::Research => "research",
            Self::Execution => "execution",
        }
    }

    pub const ALL: [Self; 5] = [
        Self::Chat,
        Self::Fact,
        Self::Decision,
        Self::Research,
        Self::Execution,
    ];
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord)]
#[serde(rename_all = "snake_case")]
pub enum AnswerForm {
    Direct,
    Decision,
    Research,
    Execution,
}

impl AnswerForm {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Direct => "direct",
            Self::Decision => "decision",
            Self::Research => "research",
            Self::Execution => "execution",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct GoldenPrompt {
    pub id: String,
    pub category: GoldenCategory,
    pub form: AnswerForm,
    pub prompt: String,
    #[serde(default)]
    pub notes: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct GoldenSetReport {
    pub count: u64,
    pub categories: BTreeMap<String, u64>,
    pub forms: BTreeMap<String, u64>,
}

/// Parse and validate a JSONL golden set.
///
/// Blank lines are ignored. Every non-blank line must be a complete
/// `GoldenPrompt`; ids and prompts must be unique, all five categories must be
/// present, and the total must be within `MIN..=MAX`.
pub fn parse_golden_set(text: &str) -> Result<Vec<GoldenPrompt>, String> {
    let mut prompts = Vec::new();
    let mut ids = BTreeSet::new();
    let mut contents = BTreeSet::new();
    for (index, line) in text.lines().enumerate() {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        let prompt: GoldenPrompt =
            serde_json::from_str(line).map_err(|error| format!("line {}: {error}", index + 1))?;
        validate_prompt(&prompt, index + 1)?;
        if !ids.insert(prompt.id.clone()) {
            return Err(format!("line {}: duplicate id `{}`", index + 1, prompt.id));
        }
        if !contents.insert(prompt.prompt.clone()) {
            return Err(format!("line {}: duplicate prompt", index + 1));
        }
        prompts.push(prompt);
    }
    if !(MIN_GOLDEN_PROMPTS..=MAX_GOLDEN_PROMPTS).contains(&prompts.len()) {
        return Err(format!(
            "golden set must contain {MIN_GOLDEN_PROMPTS}..={MAX_GOLDEN_PROMPTS} prompts, found {}",
            prompts.len()
        ));
    }
    for category in GoldenCategory::ALL {
        if !prompts.iter().any(|prompt| prompt.category == category) {
            return Err(format!(
                "golden set is missing category `{}`",
                category.as_str()
            ));
        }
    }
    Ok(prompts)
}

fn validate_prompt(prompt: &GoldenPrompt, line: usize) -> Result<(), String> {
    if prompt.id.is_empty() || prompt.id.chars().count() > 64 {
        return Err(format!("line {line}: id must be 1..=64 characters"));
    }
    if !prompt
        .id
        .chars()
        .all(|character| character.is_ascii_alphanumeric() || matches!(character, '-' | '_' | '.'))
    {
        return Err(format!(
            "line {line}: id must use ASCII letters, digits, `-`, `_` or `.`"
        ));
    }
    if prompt.prompt.trim().is_empty() || prompt.prompt.chars().count() > 2_000 {
        return Err(format!("line {line}: prompt must be 1..=2000 characters"));
    }
    if prompt
        .prompt
        .chars()
        .chain(prompt.notes.chars())
        .any(crate::safety::is_disallowed_control)
    {
        return Err(format!(
            "line {line}: prompt or notes contain a control character"
        ));
    }
    Ok(())
}

pub fn summarize_golden_set(prompts: &[GoldenPrompt]) -> GoldenSetReport {
    let mut categories = BTreeMap::new();
    let mut forms = BTreeMap::new();
    for prompt in prompts {
        *categories
            .entry(prompt.category.as_str().to_string())
            .or_insert(0u64) += 1;
        *forms
            .entry(prompt.form.as_str().to_string())
            .or_insert(0u64) += 1;
    }
    GoldenSetReport {
        count: prompts.len() as u64,
        categories,
        forms,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn prompt(id: &str, category: GoldenCategory) -> GoldenPrompt {
        GoldenPrompt {
            id: id.into(),
            category,
            form: AnswerForm::Direct,
            prompt: format!("prompt {id}"),
            notes: String::new(),
        }
    }

    fn jsonl(prompts: &[GoldenPrompt]) -> String {
        prompts
            .iter()
            .map(|prompt| serde_json::to_string(prompt).unwrap())
            .collect::<Vec<_>>()
            .join("\n")
    }

    fn minimum_set() -> Vec<GoldenPrompt> {
        let mut prompts = Vec::new();
        for (index, category) in GoldenCategory::ALL.iter().enumerate() {
            for offset in 0..6 {
                prompts.push(prompt(&format!("p{index}-{offset}"), *category));
            }
        }
        prompts
    }

    #[test]
    fn shipped_golden_set_is_valid() {
        let prompts = parse_golden_set(include_str!("../evals/golden-set-v1.jsonl")).unwrap();
        assert_eq!(prompts.len(), 40);
        let report = summarize_golden_set(&prompts);
        assert_eq!(report.count, 40);
        for category in GoldenCategory::ALL {
            assert_eq!(report.categories.get(category.as_str()), Some(&8));
        }
    }

    #[test]
    fn valid_minimum_set_is_accepted() {
        let prompts = minimum_set();
        let parsed = parse_golden_set(&jsonl(&prompts)).unwrap();
        assert_eq!(parsed.len(), 30);
    }

    #[test]
    fn duplicate_ids_and_prompts_are_rejected() {
        let mut prompts = minimum_set();
        prompts[1].id = prompts[0].id.clone();
        let error = parse_golden_set(&jsonl(&prompts)).unwrap_err();
        assert!(error.contains("duplicate id"), "{error}");

        let mut prompts = minimum_set();
        prompts[1].prompt = prompts[0].prompt.clone();
        let error = parse_golden_set(&jsonl(&prompts)).unwrap_err();
        assert!(error.contains("duplicate prompt"), "{error}");
    }

    #[test]
    fn count_and_category_coverage_are_enforced() {
        let prompts = minimum_set();
        let error = parse_golden_set(&jsonl(&prompts[..29])).unwrap_err();
        assert!(error.contains("30..=50"), "{error}");

        let mut prompts = minimum_set();
        prompts.retain(|prompt| prompt.category != GoldenCategory::Execution);
        while prompts.len() < 30 {
            prompts.push(prompt(
                &format!("extra-{}", prompts.len()),
                GoldenCategory::Chat,
            ));
        }
        let error = parse_golden_set(&jsonl(&prompts)).unwrap_err();
        assert!(error.contains("missing category `execution`"), "{error}");
    }

    #[test]
    fn invalid_fields_fail_closed() {
        let mut prompts = minimum_set();
        prompts[0].id = "bad id".into();
        let error = parse_golden_set(&jsonl(&prompts)).unwrap_err();
        assert!(error.contains("ASCII letters"), "{error}");

        let mut prompts = minimum_set();
        prompts[0].prompt = "bad\u{0007}prompt".into();
        let error = parse_golden_set(&jsonl(&prompts)).unwrap_err();
        assert!(error.contains("control character"), "{error}");
    }

    #[test]
    fn malformed_json_reports_the_line() {
        let mut text = jsonl(&minimum_set());
        text.push_str("\n{\"id\":\"broken\"}");
        let error = parse_golden_set(&text).unwrap_err();
        assert!(error.contains("line 31:"), "{error}");
    }
}
