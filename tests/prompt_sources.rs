use std::collections::BTreeMap;

use first_principles_agent::tui_prompt_sources::PROMPT_SOURCE_GROUPS;
use serde_json::Value;

fn first(record: &Value, field: &str) -> String {
    match record.get(field) {
        Some(Value::Array(values)) => values
            .first()
            .and_then(Value::as_str)
            .unwrap_or_default()
            .trim()
            .to_string(),
        Some(Value::String(value)) => value.trim().to_string(),
        _ => String::new(),
    }
}

#[test]
fn source_groups_keep_every_table_row_complete() {
    let raw = include_str!("../docs/evidence/prompt-sources-2026-11-11.ndjson");
    let mut groups: BTreeMap<(String, String), Vec<Value>> = BTreeMap::new();
    for line in raw.lines().filter(|line| !line.trim().is_empty()) {
        let record: Value = serde_json::from_str(line).expect("valid snapshot row");
        let author = first(&record, "作者");
        let topic = first(&record, "主题");
        groups.entry((author, topic)).or_default().push(record);
    }

    assert_eq!(groups.len(), PROMPT_SOURCE_GROUPS.len());
    for ((author, topic), records) in groups {
        let group = PROMPT_SOURCE_GROUPS
            .iter()
            .find(|group| group.author == author && group.topic == topic)
            .unwrap_or_else(|| panic!("missing source group {author} / {topic}"));
        assert_eq!(group.fragments, records.len(), "{author} / {topic}");
        let original = records
            .iter()
            .map(|record| first(record, "原文（英文逐字）"))
            .filter(|text| !text.is_empty())
            .collect::<Vec<_>>()
            .join("\n\n");
        let translation = records
            .iter()
            .map(|record| first(record, "中文直译"))
            .filter(|text| !text.is_empty())
            .collect::<Vec<_>>()
            .join("\n\n");
        assert_eq!(group.original, original, "{author} / {topic} original");
        assert_eq!(
            group.translation, translation,
            "{author} / {topic} translation"
        );
    }
}
