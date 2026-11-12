//! Capture a completed run journal into a scannable `SKILL.md` draft.
//!
//! The draft is deliberately not registered or activated. A human reviews the
//! redacted procedure, runs held-out evaluation and activates it through the
//! existing candidate lifecycle.

use std::collections::{BTreeMap, BTreeSet};
use std::fs::File;
use std::io::{BufReader, Read};
use std::path::Path;

use serde_json::Value;
use thiserror::Error;
use uuid::Uuid;

use crate::domain::{StopReason, ToolStatus};
use crate::journal::{JournalEvent, JournalEventKind};
use crate::skill::{SkillManifest, scan_markdown};

pub const MAX_CAPTURE_JOURNAL_BYTES: u64 = 64 * 1024 * 1024;
pub const MAX_CAPTURE_STEPS: usize = 100;
const MAX_STEP_CHARS: usize = 240;

#[derive(Debug, Error, PartialEq, Eq)]
pub enum SkillCaptureError {
    #[error("skill capture journal is too large: {0} bytes (limit {MAX_CAPTURE_JOURNAL_BYTES})")]
    JournalTooLarge(u64),
    #[error("skill capture journal read failed: {0}")]
    Journal(String),
    #[error("skill capture requires a completed run")]
    NotCompleted,
    #[error("skill capture found no successful tool step")]
    NoSuccessfulSteps,
    #[error("skill capture found {0} successful steps (limit {MAX_CAPTURE_STEPS})")]
    TooManySteps(usize),
    #[error("skill capture produced an invalid SKILL.md: {0}")]
    InvalidDraft(String),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CapturedSkill {
    pub manifest: SkillManifest,
    pub markdown: String,
    pub run_id: Uuid,
    pub steps: usize,
    pub redactions: usize,
    pub generalizations: usize,
}

/// Build a draft from the last run segment in `path`. Only a `Completed` stop
/// is accepted, and only tool calls with an `Ok` observation become steps.
pub fn capture_journal(
    path: impl AsRef<Path>,
    name: &str,
    version: &str,
    description: &str,
    scope: &str,
) -> Result<CapturedSkill, SkillCaptureError> {
    for (label, value) in [
        ("name", name),
        ("version", version),
        ("description", description),
        ("scope", scope),
    ] {
        if value.trim().is_empty() || value.chars().any(char::is_control) {
            return Err(SkillCaptureError::InvalidDraft(format!(
                "{label} must be a non-empty single line"
            )));
        }
    }
    let path = path.as_ref();
    let metadata =
        std::fs::metadata(path).map_err(|error| SkillCaptureError::Journal(error.to_string()))?;
    if !metadata.is_file() {
        return Err(SkillCaptureError::Journal(
            "journal is not a regular file".into(),
        ));
    }
    if metadata.len() > MAX_CAPTURE_JOURNAL_BYTES {
        return Err(SkillCaptureError::JournalTooLarge(metadata.len()));
    }
    let file = File::open(path).map_err(|error| SkillCaptureError::Journal(error.to_string()))?;
    let mut reader = BufReader::new(file).take(MAX_CAPTURE_JOURNAL_BYTES + 1);
    let mut raw = String::new();
    reader
        .read_to_string(&mut raw)
        .map_err(|error| SkillCaptureError::Journal(error.to_string()))?;
    if raw.len() as u64 > MAX_CAPTURE_JOURNAL_BYTES {
        return Err(SkillCaptureError::JournalTooLarge(raw.len() as u64));
    }
    let mut events = Vec::new();
    for (index, line) in raw.lines().enumerate() {
        if line.trim().is_empty() {
            continue;
        }
        let event = serde_json::from_str::<JournalEvent>(line)
            .map_err(|error| SkillCaptureError::Journal(format!("line {}: {error}", index + 1)))?;
        events.push(event);
    }

    let start = events
        .iter()
        .rposition(|event| {
            matches!(
                event.event,
                JournalEventKind::RunStarted { .. } | JournalEventKind::ResumeStarted { .. }
            )
        })
        .ok_or(SkillCaptureError::NotCompleted)?;
    let objective = match &events[start].event {
        JournalEventKind::RunStarted { request, .. }
        | JournalEventKind::ResumeStarted { request, .. } => request.objective.clone(),
        _ => unreachable!("start index points at a run start"),
    };
    let run_id = events[start].run_id;
    let stop = events[start + 1..]
        .iter()
        .position(|event| {
            matches!(
                event.event,
                JournalEventKind::RunStopped {
                    reason: StopReason::Completed
                }
            )
        })
        .map(|position| start + 1 + position)
        .ok_or(SkillCaptureError::NotCompleted)?;

    let mut observed = BTreeMap::<(String, String), ToolStatus>::new();
    for event in &events[start + 1..stop] {
        if let JournalEventKind::ToolObserved { observation } = &event.event {
            observed.insert(
                (observation.call_id.clone(), observation.tool.clone()),
                observation.status,
            );
        }
    }
    let mut seen = BTreeSet::new();
    let mut calls = Vec::new();
    for event in &events[start + 1..stop] {
        if let JournalEventKind::ToolCalled { call } = &event.event {
            if !seen.insert(call.id.clone()) {
                continue;
            }
            if observed.get(&(call.id.clone(), call.name.clone())) == Some(&ToolStatus::Ok) {
                calls.push(call.clone());
            }
        }
    }
    if calls.is_empty() {
        return Err(SkillCaptureError::NoSuccessfulSteps);
    }
    if calls.len() > MAX_CAPTURE_STEPS {
        return Err(SkillCaptureError::TooManySteps(calls.len()));
    }

    let mut redactions = 0;
    let mut generalizations = 0;
    let objective = sanitize_text(&objective, &mut redactions, &mut generalizations);
    let mut tools = BTreeSet::new();
    let mut procedure = Vec::with_capacity(calls.len());
    for (index, call) in calls.iter().enumerate() {
        if call.name.trim().is_empty() || call.name.chars().any(char::is_control) {
            return Err(SkillCaptureError::InvalidDraft(
                "tool name must be a non-empty single line".into(),
            ));
        }
        tools.insert(call.name.clone());
        let value = sanitize_value(&call.arguments, &mut redactions, &mut generalizations);
        let arguments = serde_json::to_string(&value).unwrap_or_else(|_| "{}".into());
        let arguments = truncate_chars(&arguments, MAX_STEP_CHARS);
        procedure.push(format!("{}. `{}` — `{}`", index + 1, call.name, arguments));
    }

    let markdown = format!(
        "---\nname: {name}\ndescription: {description}\nversion: {version}\nscope: {scope}\nrequires_tools: {}\nrisk: medium\n---\n\n# {name}\n\n## When to use\n{objective}\n\n## Procedure\n{}\n\n## Verification\n- Replay the original task and confirm the same deliverable.\n- Review that no run-specific values remain: `$HOME`, `$TMPDIR`, `<id>`, `<email>`, `<redacted>`.\n\n## Provenance\n- source run: {run_id}\n- captured steps: {}\n",
        tools.into_iter().collect::<Vec<_>>().join(", "),
        procedure.join("\n"),
        calls.len()
    );
    let manifest = scan_markdown(&markdown)
        .map_err(|error| SkillCaptureError::InvalidDraft(error.to_string()))?;
    Ok(CapturedSkill {
        manifest,
        markdown,
        run_id,
        steps: calls.len(),
        redactions,
        generalizations,
    })
}

fn sanitize_value(value: &Value, redactions: &mut usize, generalizations: &mut usize) -> Value {
    match value {
        Value::Object(map) => {
            let mut output = serde_json::Map::new();
            let mut next_suffix = BTreeMap::<String, usize>::new();
            for (key, value) in map {
                let display_key = sanitize_key(key, redactions, generalizations);
                let value = if is_secret_key(key) {
                    *redactions += 1;
                    Value::String("<redacted>".into())
                } else {
                    sanitize_value(value, redactions, generalizations)
                };
                let mut unique = display_key.clone();
                if output.contains_key(&unique) {
                    let counter = next_suffix.entry(display_key.clone()).or_insert(2);
                    loop {
                        unique = format!("{display_key}#{counter}");
                        *counter += 1;
                        if !output.contains_key(&unique) {
                            break;
                        }
                    }
                }
                output.insert(unique, value);
            }
            Value::Object(output)
        }
        Value::Array(items) => Value::Array(
            items
                .iter()
                .map(|item| sanitize_value(item, redactions, generalizations))
                .collect(),
        ),
        Value::String(text) => Value::String(sanitize_text(text, redactions, generalizations)),
        other => other.clone(),
    }
}

fn sanitize_key(key: &str, redactions: &mut usize, generalizations: &mut usize) -> String {
    let escaped = key.replace('<', "&lt;").replace('>', "&gt;");
    let key = truncate_chars(&sanitize_text(&escaped, redactions, generalizations), 120);
    if key.is_empty() { "<key>".into() } else { key }
}

fn is_secret_key(key: &str) -> bool {
    let key = key.to_ascii_lowercase().replace('-', "_");
    [
        "token",
        "secret",
        "password",
        "api_key",
        "apikey",
        "access_key",
        "private_key",
        "authorization",
        "credential",
    ]
    .iter()
    .any(|marker| key.contains(marker))
}

fn sanitize_text(input: &str, redactions: &mut usize, generalizations: &mut usize) -> String {
    let mut text = collapse_controls(input);
    text = redact_prefixed_secrets(&text, redactions);
    text = redact_bearer(&text, redactions);
    text = redact_assignments(&text, redactions);
    text = redact_url_userinfo(&text, redactions);
    text = generalize_home(&text, generalizations);
    text = generalize_temp(&text, generalizations);
    text = generalize_uuids(&text, generalizations);
    generalize_emails(&text, generalizations)
}

fn collapse_controls(input: &str) -> String {
    let mut output = String::with_capacity(input.len());
    let mut last_space = false;
    for character in input.chars() {
        let character = if character.is_control() {
            ' '
        } else if character == '`' {
            '\''
        } else {
            character
        };
        if character == ' ' {
            if !last_space && !output.is_empty() {
                output.push(' ');
            }
            last_space = true;
        } else {
            output.push(character);
            last_space = false;
        }
    }
    output.trim().to_string()
}

const SECRET_PREFIXES: &[(&str, usize)] = &[
    ("sk-", 12),
    ("ghp_", 8),
    ("gho_", 8),
    ("ghu_", 8),
    ("ghs_", 8),
    ("ghr_", 8),
    ("github_pat_", 8),
    ("xoxb-", 8),
    ("xoxa-", 8),
    ("xoxp-", 8),
    ("xoxr-", 8),
    ("xoxs-", 8),
    ("AKIA", 16),
    ("ASIA", 16),
];

fn redact_prefixed_secrets(input: &str, redactions: &mut usize) -> String {
    let mut output = String::with_capacity(input.len());
    let mut index = 0;
    while index < input.len() {
        let matched = SECRET_PREFIXES.iter().find(|(prefix, minimum)| {
            let boundary = index == 0
                || !input[..index]
                    .chars()
                    .next_back()
                    .is_some_and(is_token_character);
            boundary
                && input[index..].starts_with(prefix)
                && input[index + prefix.len()..]
                    .chars()
                    .take_while(|character| is_token_character(*character))
                    .count()
                    >= *minimum
        });
        if let Some((prefix, _)) = matched {
            let mut end = index + prefix.len();
            end += input[end..]
                .chars()
                .take_while(|character| is_token_character(*character))
                .map(char::len_utf8)
                .sum::<usize>();
            output.push_str("<redacted>");
            *redactions += 1;
            index = end;
            continue;
        }
        let character = input[index..].chars().next().unwrap();
        output.push(character);
        index += character.len_utf8();
    }
    output
}

fn is_token_character(character: char) -> bool {
    character.is_ascii_alphanumeric() || matches!(character, '_' | '-')
}

fn redact_bearer(input: &str, redactions: &mut usize) -> String {
    let mut output = String::with_capacity(input.len());
    let mut index = 0;
    while index < input.len() {
        if input[index..]
            .get(..7)
            .is_some_and(|prefix| prefix.eq_ignore_ascii_case("bearer "))
        {
            output.push_str(&input[index..index + 7]);
            let start = index + 7;
            let mut end = start;
            for (offset, character) in input[start..].char_indices() {
                if character.is_whitespace()
                    || matches!(character, '"' | '\'' | ',' | ';' | ')' | ']' | '}' | '\\')
                {
                    end = start + offset;
                    break;
                }
                end = start + offset + character.len_utf8();
            }
            if end > start {
                output.push_str("<redacted>");
                *redactions += 1;
                index = end;
                continue;
            }
        }
        let character = input[index..].chars().next().unwrap();
        output.push(character);
        index += character.len_utf8();
    }
    output
}

const ASSIGNMENT_KEYS: &[&str] = &[
    "token",
    "secret",
    "password",
    "api_key",
    "api-key",
    "apikey",
    "access_key",
    "private_key",
    "authorization",
    "credential",
];

fn redact_assignments(input: &str, redactions: &mut usize) -> String {
    let mut output = String::with_capacity(input.len());
    let mut index = 0;
    while index < input.len() {
        let character = input[index..].chars().next().unwrap();
        if matches!(character, '"' | '\'') {
            let quote = character;
            let key_start = index + quote.len_utf8();
            if let Some(offset) = input[key_start..].find(quote) {
                let key_end = key_start + offset;
                let key = &input[key_start..key_end];
                let lower = key.to_ascii_lowercase();
                if ASSIGNMENT_KEYS.iter().any(|marker| lower.contains(marker)) {
                    let mut cursor = key_end + quote.len_utf8();
                    while cursor < input.len()
                        && input[cursor..]
                            .chars()
                            .next()
                            .is_some_and(char::is_whitespace)
                    {
                        cursor += input[cursor..].chars().next().unwrap().len_utf8();
                    }
                    if input[cursor..].starts_with(':') || input[cursor..].starts_with('=') {
                        cursor += 1;
                        while cursor < input.len()
                            && input[cursor..]
                                .chars()
                                .next()
                                .is_some_and(char::is_whitespace)
                        {
                            cursor += input[cursor..].chars().next().unwrap().len_utf8();
                        }
                        if let Some(end) = value_end(input, cursor) {
                            output.push(quote);
                            output.push_str(key);
                            output.push(quote);
                            output.push_str(":<redacted>");
                            *redactions += 1;
                            index = end;
                            continue;
                        }
                    }
                }
            }
        }
        let starts_key = index == 0
            || !input[..index]
                .chars()
                .next_back()
                .is_some_and(|previous| previous.is_ascii_alphanumeric() || previous == '_');
        if starts_key {
            let key_len = input[index..]
                .chars()
                .take_while(|character| {
                    character.is_ascii_alphanumeric() || matches!(character, '_' | '-')
                })
                .map(char::len_utf8)
                .sum::<usize>();
            if key_len > 0 {
                let key = &input[index..index + key_len];
                let lower = key.to_ascii_lowercase();
                if ASSIGNMENT_KEYS.iter().any(|marker| lower.contains(marker)) {
                    let mut cursor = index + key_len;
                    while cursor < input.len()
                        && input[cursor..]
                            .chars()
                            .next()
                            .is_some_and(char::is_whitespace)
                    {
                        cursor += input[cursor..].chars().next().unwrap().len_utf8();
                    }
                    if input[cursor..].starts_with('=') || input[cursor..].starts_with(':') {
                        cursor += 1;
                        while cursor < input.len()
                            && input[cursor..]
                                .chars()
                                .next()
                                .is_some_and(char::is_whitespace)
                        {
                            cursor += input[cursor..].chars().next().unwrap().len_utf8();
                        }
                        let quote = input[cursor..].chars().next();
                        if matches!(quote, Some('"' | '\'')) {
                            let quote = quote.unwrap();
                            cursor += quote.len_utf8();
                            let mut end = cursor;
                            for (offset, character) in input[cursor..].char_indices() {
                                if character == quote {
                                    end = cursor + offset;
                                    break;
                                }
                                end = cursor + offset + character.len_utf8();
                            }
                            output.push_str(key);
                            output.push_str("=<redacted>");
                            *redactions += 1;
                            index = end;
                            continue;
                        }
                        let mut end = cursor;
                        for (offset, character) in input[cursor..].char_indices() {
                            if character.is_whitespace()
                                || matches!(character, ',' | ';' | ')' | ']' | '}')
                            {
                                end = cursor + offset;
                                break;
                            }
                            end = cursor + offset + character.len_utf8();
                        }
                        if end > cursor {
                            output.push_str(key);
                            output.push_str("=<redacted>");
                            *redactions += 1;
                            index = end;
                            continue;
                        }
                    }
                }
            }
        }
        output.push(character);
        index += character.len_utf8();
    }
    output
}

fn value_end(input: &str, start: usize) -> Option<usize> {
    let first = input[start..].chars().next()?;
    if matches!(first, '"' | '\'') {
        let body_start = start + first.len_utf8();
        let mut escaped = false;
        for (offset, character) in input[body_start..].char_indices() {
            if escaped {
                escaped = false;
                continue;
            }
            if character == '\\' {
                escaped = true;
                continue;
            }
            if character == first {
                return Some(body_start + offset + first.len_utf8());
            }
        }
        return Some(input.len());
    }
    if matches!(first, '{' | '[') {
        let mut depth = 0usize;
        let mut quote = None;
        let mut escaped = false;
        for (offset, character) in input[start..].char_indices() {
            if let Some(open) = quote {
                if escaped {
                    escaped = false;
                } else if character == '\\' {
                    escaped = true;
                } else if character == open {
                    quote = None;
                }
                continue;
            }
            match character {
                '"' | '\'' => {
                    quote = Some(character);
                    escaped = false;
                }
                '{' | '[' => depth += 1,
                '}' | ']' => {
                    depth = depth.saturating_sub(1);
                    if depth == 0 {
                        return Some(start + offset + character.len_utf8());
                    }
                }
                _ => {}
            }
        }
        return Some(input.len());
    }
    let mut end = start;
    for (offset, character) in input[start..].char_indices() {
        if character.is_whitespace() || matches!(character, ',' | ';' | ')' | ']' | '}') {
            return Some(start + offset);
        }
        end = start + offset + character.len_utf8();
    }
    (end > start).then_some(end)
}

fn redact_url_userinfo(input: &str, redactions: &mut usize) -> String {
    let mut output = String::with_capacity(input.len());
    let mut index = 0;
    while index < input.len() {
        if input[index..].starts_with("://") {
            output.push_str("://");
            let start = index + 3;
            let mut at = None;
            for (offset, character) in input[start..].char_indices() {
                if character == '@' {
                    at = Some(start + offset);
                    continue;
                }
                if matches!(
                    character,
                    '/' | '?' | '#' | '"' | '\'' | ',' | ')' | ']' | '}' | '\\'
                ) || character.is_whitespace()
                {
                    break;
                }
            }
            if let Some(at) = at {
                output.push_str("<redacted>@");
                *redactions += 1;
                index = at + 1;
            } else {
                index = start;
            }
            continue;
        }
        let character = input[index..].chars().next().unwrap();
        output.push(character);
        index += character.len_utf8();
    }
    output
}

fn generalize_home(input: &str, generalizations: &mut usize) -> String {
    let mut output = String::with_capacity(input.len());
    let mut index = 0;
    while index < input.len() {
        let prefix_len = if input[index..].starts_with("/home/") {
            Some(6)
        } else if input[index..].starts_with("/Users/") {
            Some(7)
        } else if input
            .get(index..index + 9)
            .is_some_and(|prefix| prefix.eq_ignore_ascii_case("C:\\Users\\"))
        {
            Some(9)
        } else {
            None
        };
        if let Some(prefix_len) = prefix_len {
            let start = index + prefix_len;
            let mut end = start;
            for (offset, character) in input[start..].char_indices() {
                if character == '/'
                    || character.is_whitespace()
                    || matches!(character, '"' | '\'' | ',' | ';' | ')' | ']' | '}' | '\\')
                {
                    end = start + offset;
                    break;
                }
                end = start + offset + character.len_utf8();
            }
            if end > start {
                output.push_str("$HOME");
                *generalizations += 1;
                index = end;
                continue;
            }
        }
        let character = input[index..].chars().next().unwrap();
        output.push(character);
        index += character.len_utf8();
    }
    output
}

fn generalize_temp(input: &str, generalizations: &mut usize) -> String {
    let mut output = input.to_string();
    for prefix in ["/var/tmp/", "/tmp/"] {
        let count = output.matches(prefix).count();
        if count > 0 {
            output = output.replace(prefix, "$TMPDIR/");
            *generalizations += count;
        }
    }
    output
}

fn generalize_uuids(input: &str, generalizations: &mut usize) -> String {
    let mut output = String::with_capacity(input.len());
    let mut index = 0;
    while index < input.len() {
        if let Some(length) = uuid_length(&input[index..]) {
            output.push_str("<id>");
            *generalizations += 1;
            index += length;
            continue;
        }
        let character = input[index..].chars().next().unwrap();
        output.push(character);
        index += character.len_utf8();
    }
    output
}

fn uuid_length(input: &str) -> Option<usize> {
    let bytes = input.as_bytes();
    if bytes.len() < 36 {
        return None;
    }
    for (index, byte) in bytes[..36].iter().enumerate() {
        let valid = if matches!(index, 8 | 13 | 18 | 23) {
            *byte == b'-'
        } else {
            byte.is_ascii_hexdigit()
        };
        if !valid {
            return None;
        }
    }
    if bytes.get(36).is_some_and(|byte| byte.is_ascii_hexdigit()) {
        return None;
    }
    Some(36)
}

fn generalize_emails(input: &str, generalizations: &mut usize) -> String {
    let mut output = String::with_capacity(input.len());
    let mut token = String::new();
    for character in input.chars() {
        if character.is_whitespace()
            || matches!(
                character,
                '"' | '\'' | ',' | ';' | '(' | ')' | '[' | ']' | '{' | '}' | '<' | '>'
            )
        {
            flush_email(&mut token, &mut output, generalizations);
            output.push(character);
        } else {
            token.push(character);
        }
    }
    flush_email(&mut token, &mut output, generalizations);
    output
}

fn flush_email(token: &mut String, output: &mut String, generalizations: &mut usize) {
    let mut end = token.len();
    while let Some((index, character)) = token[..end].char_indices().next_back() {
        if matches!(character, '.' | ':' | '!' | '?' | ';' | ',') {
            end = index;
        } else {
            break;
        }
    }
    let (core, suffix) = token.split_at(end);
    if is_email(core) {
        output.push_str("<email>");
        output.push_str(suffix);
        *generalizations += 1;
    } else {
        output.push_str(token);
    }
    token.clear();
}

fn is_email(token: &str) -> bool {
    let Some((local, domain)) = token.split_once('@') else {
        return false;
    };
    !local.is_empty()
        && domain.contains('.')
        && !domain.starts_with('.')
        && !domain.ends_with('.')
        && !domain.contains('@')
}

fn truncate_chars(input: &str, limit: usize) -> String {
    let mut output = input.chars().take(limit).collect::<String>();
    if input.chars().count() > limit {
        output.push('…');
    }
    output
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::{CognitiveMode, TaskRequest, ToolCall, ToolObservation};
    use crate::journal::JournalEvent;
    use serde_json::json;
    use std::io::Write;
    use std::path::PathBuf;

    fn event(run_id: Uuid, seq: u64, event: JournalEventKind) -> JournalEvent {
        JournalEvent {
            schema_version: 1,
            run_id,
            seq,
            timestamp_unix_ms: seq as u128,
            event,
        }
    }

    fn write_journal(path: &Path, events: &[JournalEvent]) {
        let mut file = std::fs::File::create(path).unwrap();
        for event in events {
            writeln!(file, "{}", serde_json::to_string(event).unwrap()).unwrap();
        }
    }

    fn completed_run(path: &Path) -> Uuid {
        let run_id = Uuid::new_v4();
        let request = TaskRequest::from_objective(
            "Follow up on /home/alice/acme with bob@example.com",
            PathBuf::from("/tmp"),
        );
        let events = vec![
            event(
                run_id,
                0,
                JournalEventKind::RunStarted {
                    request,
                    mode: CognitiveMode::Execute,
                },
            ),
            event(
                run_id,
                1,
                JournalEventKind::ToolCalled {
                    call: ToolCall {
                        id: "call-1".into(),
                        name: "read_file".into(),
                        arguments: json!({
                            "path": "/home/alice/acme/README.md",
                            "api_key": "sk-ant-1234567890abcdefghij",
                            "comment": "token sk-ant-1234567890abcdefghij",
                            "note": "id 550e8400-e29b-41d4-a716-446655440000",
                            "headers": {"X-Api-Key": "SECRET-HEADER-1234"},
                            "nested": {"token": {"a": "b", "c": "SECRET-ONE"}},
                            "users": {"bob@example.com": 1},
                            "paths": {"/home/alice/x": 2},
                            "a`b": 3
                        }),
                    },
                },
            ),
            event(
                run_id,
                2,
                JournalEventKind::ToolObserved {
                    observation: ToolObservation {
                        call_id: "call-1".into(),
                        tool: "read_file".into(),
                        status: ToolStatus::Ok,
                        summary: "read".into(),
                        content: "secret content must not be captured".into(),
                        error_code: None,
                        metadata: BTreeMap::new(),
                    },
                },
            ),
            event(
                run_id,
                3,
                JournalEventKind::RunStopped {
                    reason: StopReason::Completed,
                },
            ),
        ];
        write_journal(path, &events);
        run_id
    }

    #[test]
    fn captures_a_completed_run_with_redaction_and_generalization() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("run.jsonl");
        let run_id = completed_run(&path);

        let captured = capture_journal(
            &path,
            "follow-up",
            "1.0.0",
            "Follow up on a project",
            "global",
        )
        .unwrap();
        assert_eq!(captured.run_id, run_id);
        assert_eq!(captured.steps, 1);
        assert!(captured.redactions >= 4);
        assert!(captured.generalizations >= 4);
        assert!(captured.markdown.contains("$HOME/acme/README.md"));
        assert!(captured.markdown.contains("$HOME/x"));
        assert!(captured.markdown.contains("<redacted>"));
        assert!(captured.markdown.contains("<id>"));
        assert!(captured.markdown.contains("<email>"));
        assert!(captured.markdown.contains("a'b"));
        assert!(!captured.markdown.contains("/home/alice"));
        assert!(!captured.markdown.contains("bob@example.com"));
        assert!(!captured.markdown.contains("SECRET-HEADER-1234"));
        assert!(!captured.markdown.contains("SECRET-ONE"));
        assert!(!captured.markdown.contains("sk-ant-1234567890abcdefghij"));
        assert!(
            !captured
                .markdown
                .contains("550e8400-e29b-41d4-a716-446655440000")
        );
        assert!(!captured.markdown.contains("secret content"));
        assert_eq!(captured.manifest.name, "follow-up");
        assert_eq!(captured.manifest.requires_tools, vec!["read_file"]);
        assert_eq!(captured.manifest.risk, crate::skill::SkillRisk::Medium);
    }

    #[test]
    fn refuses_a_run_without_a_completed_stop() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("run.jsonl");
        let run_id = Uuid::new_v4();
        let events = vec![
            event(
                run_id,
                0,
                JournalEventKind::RunStarted {
                    request: TaskRequest::from_objective("task", PathBuf::from("/tmp")),
                    mode: CognitiveMode::Execute,
                },
            ),
            event(
                run_id,
                1,
                JournalEventKind::RunStopped {
                    reason: StopReason::Cancelled,
                },
            ),
        ];
        write_journal(&path, &events);
        assert_eq!(
            capture_journal(&path, "draft", "1.0.0", "Draft", "global").unwrap_err(),
            SkillCaptureError::NotCompleted
        );
    }

    #[test]
    fn refuses_a_run_without_successful_tools() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("run.jsonl");
        let run_id = Uuid::new_v4();
        let events = vec![
            event(
                run_id,
                0,
                JournalEventKind::RunStarted {
                    request: TaskRequest::from_objective("task", PathBuf::from("/tmp")),
                    mode: CognitiveMode::Execute,
                },
            ),
            event(
                run_id,
                1,
                JournalEventKind::ToolCalled {
                    call: ToolCall {
                        id: "call-1".into(),
                        name: "read_file".into(),
                        arguments: json!({"path": "/tmp/a"}),
                    },
                },
            ),
            event(
                run_id,
                2,
                JournalEventKind::ToolObserved {
                    observation: ToolObservation {
                        call_id: "call-1".into(),
                        tool: "read_file".into(),
                        status: ToolStatus::Failed,
                        summary: "failed".into(),
                        content: String::new(),
                        error_code: Some("io".into()),
                        metadata: BTreeMap::new(),
                    },
                },
            ),
            event(
                run_id,
                3,
                JournalEventKind::RunStopped {
                    reason: StopReason::Completed,
                },
            ),
        ];
        write_journal(&path, &events);
        assert_eq!(
            capture_journal(&path, "draft", "1.0.0", "Draft", "global").unwrap_err(),
            SkillCaptureError::NoSuccessfulSteps
        );
    }

    #[test]
    fn redacts_escaped_quotes_inside_secret_values() {
        let mut redactions = 0;
        let mut generalizations = 0;
        for raw in [
            r#"{"api_key":"safe\"LEAK-QUOTED"}"#,
            r#"{"api_key":{"a":"x\"},"z":"LEAK-NESTED"}}"#,
        ] {
            let text = sanitize_text(raw, &mut redactions, &mut generalizations);
            assert!(!text.contains("LEAK-QUOTED"), "{text}");
            assert!(!text.contains("LEAK-NESTED"), "{text}");
        }
    }

    #[test]
    fn sanitizes_secret_named_keys_and_avoids_quadratic_collisions() {
        let mut redactions = 0;
        let mut generalizations = 0;
        let value = sanitize_value(
            &json!({
                "x-token`<script>alert(1)</script>": "v",
                "a@example.com": 1,
                "b@example.com": 2,
                "c@example.com": 3
            }),
            &mut redactions,
            &mut generalizations,
        );
        let encoded = serde_json::to_string(&value).unwrap();
        assert!(!encoded.contains("<script>"), "{encoded}");
        assert!(encoded.contains("&lt;script&gt;"), "{encoded}");
        assert!(encoded.contains("\"<email>\""), "{encoded}");
        assert!(encoded.contains("\"<email>#2\""), "{encoded}");
        assert!(encoded.contains("\"<email>#3\""), "{encoded}");
    }

    #[test]
    fn rejects_frontmatter_injection_in_metadata() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("run.jsonl");
        completed_run(&path);
        for (name, version, description, scope) in [
            ("draft", "1.0.0", "Draft", "global\nname: hijacked"),
            ("draft\nname: hijacked", "1.0.0", "Draft", "global"),
            ("draft", "1.0.0", "Draft\nname: hijacked", "global"),
        ] {
            assert!(matches!(
                capture_journal(&path, name, version, description, scope),
                Err(SkillCaptureError::InvalidDraft(_))
            ));
        }
    }

    #[test]
    fn refuses_a_run_with_too_many_steps() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("run.jsonl");
        let run_id = Uuid::new_v4();
        let mut events = vec![event(
            run_id,
            0,
            JournalEventKind::RunStarted {
                request: TaskRequest::from_objective("task", PathBuf::from("/tmp")),
                mode: CognitiveMode::Execute,
            },
        )];
        for index in 0..=MAX_CAPTURE_STEPS {
            let call_id = format!("call-{index}");
            events.push(event(
                run_id,
                (2 * index + 1) as u64,
                JournalEventKind::ToolCalled {
                    call: ToolCall {
                        id: call_id.clone(),
                        name: "read_file".into(),
                        arguments: json!({"path": "/tmp/a"}),
                    },
                },
            ));
            events.push(event(
                run_id,
                (2 * index + 2) as u64,
                JournalEventKind::ToolObserved {
                    observation: ToolObservation {
                        call_id,
                        tool: "read_file".into(),
                        status: ToolStatus::Ok,
                        summary: "ok".into(),
                        content: String::new(),
                        error_code: None,
                        metadata: BTreeMap::new(),
                    },
                },
            ));
        }
        events.push(event(
            run_id,
            (2 * MAX_CAPTURE_STEPS + 3) as u64,
            JournalEventKind::RunStopped {
                reason: StopReason::Completed,
            },
        ));
        write_journal(&path, &events);
        assert_eq!(
            capture_journal(&path, "draft", "1.0.0", "Draft", "global").unwrap_err(),
            SkillCaptureError::TooManySteps(MAX_CAPTURE_STEPS + 1)
        );
    }

    #[test]
    fn rejects_an_oversized_journal_before_reading_it() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("huge.jsonl");
        let file = std::fs::File::create(&path).unwrap();
        file.set_len(MAX_CAPTURE_JOURNAL_BYTES + 1).unwrap();
        assert_eq!(
            capture_journal(&path, "draft", "1.0.0", "Draft", "global").unwrap_err(),
            SkillCaptureError::JournalTooLarge(MAX_CAPTURE_JOURNAL_BYTES + 1)
        );
    }

    #[test]
    fn sanitizes_assignments_and_url_credentials() {
        let mut redactions = 0;
        let mut generalizations = 0;
        let text = sanitize_text(
            "password=hunter2 password: hunter3 Bearer abc.def \
             https://alice:p@ssw0rd@example.com/x /tmp/build token=abc \
             {\"api_key\":{\"a\":\"b\",\"c\":\"SECRET-ONE\"}} \
             {\"token\":[1,2,\"SECRET-TWO\"]} contact bob@example.com. \
             \"api-key\": \"quoted-secret\" C:\\Users\\bob\\project",
            &mut redactions,
            &mut generalizations,
        );
        assert!(!text.contains("hunter2"));
        assert!(!text.contains("hunter3"));
        assert!(!text.contains("abc.def"));
        assert!(!text.contains("user:pass"));
        assert!(!text.contains("ssw0rd"));
        assert!(!text.contains("quoted-secret"));
        assert!(!text.contains("SECRET-ONE"));
        assert!(!text.contains("SECRET-TWO"));
        assert!(text.contains("$TMPDIR/build"));
        assert!(text.contains("$HOME"));
        assert!(text.contains("<email>."));
        assert!(text.matches("<redacted>").count() >= 8);
        assert!(redactions >= 8);
        assert!(generalizations >= 2);
    }
}
