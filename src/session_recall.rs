//! Cross-session recall over the local TUI session store.
//!
//! The TUI groups run journals into sessions. This module is the read side:
//! it searches session titles plus the user request and answer conclusion of
//! each recorded run. Tool observations are parsed while replaying a journal
//! but are never used or returned. It never edits, forgets or reinterprets a
//! record.
//!
//! Recall is deliberately evidence-shaped. A hit carries the session title and
//! the exact snippets that matched, so the model can cite what was said or say
//! plainly that no local record matched. There is no embedding model and no
//! claim that a retrieved record is still true.

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};
use std::time::UNIX_EPOCH;

use serde::Deserialize;
use thiserror::Error;

use crate::journal::{read_events, replay_events};

const MAX_SESSIONS_SCANNED: usize = 256;
const MAX_JOURNALS_PER_SESSION: usize = 32;
const MAX_SESSION_FILE_BYTES: u64 = 4 * 1024 * 1024;
const MAX_JOURNAL_BYTES: u64 = 8 * 1024 * 1024;
const MAX_TOTAL_JOURNAL_BYTES: u64 = 64 * 1024 * 1024;
const MAX_EVIDENCE_PER_SESSION: usize = 2;
const MAX_REQUEST_CHARS: usize = 700;
const MAX_CONCLUSION_CHARS: usize = 900;
const MIN_DISTINCTIVE_SCORE: u32 = 3;
const MIN_SINGLE_LETTER_SCORE: u32 = 8;
const TITLE_WEIGHT: u32 = 2;
const REQUEST_WEIGHT: u32 = 1;
const CONCLUSION_WEIGHT: u32 = 1;
const FULL_PHRASE_BONUS: u32 = 20;

#[derive(Debug, Error)]
pub enum SessionRecallError {
    #[error("session store io: {0}")]
    Io(#[from] std::io::Error),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SessionRecallEvidence {
    pub run_id: String,
    pub timestamp_unix_ms: u128,
    pub request: String,
    pub conclusion: String,
    pub status: String,
    pub score: u32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SessionRecallHit {
    pub session_id: String,
    pub title: String,
    pub last_used_unix_ms: u128,
    pub runs: usize,
    pub score: u32,
    pub matched_terms: Vec<String>,
    pub evidence: Vec<SessionRecallEvidence>,
}

#[derive(Debug, Deserialize)]
struct SessionMeta {
    id: String,
    #[serde(default)]
    title: String,
    #[serde(default)]
    journals: Vec<PathBuf>,
}

#[derive(Debug, Default)]
struct QueryTerms {
    ascii: BTreeSet<String>,
    cjk: BTreeSet<String>,
}

impl QueryTerms {
    fn is_empty(&self) -> bool {
        self.ascii.is_empty() && self.cjk.is_empty()
    }
}

/// Search earlier sessions and return bounded, evidence-bearing hits.
///
/// `current_session_id` is excluded so the current conversation cannot recall
/// itself. An empty query (or a query made only of memory small talk) lists the
/// most recent sessions instead of returning nothing.
pub fn recall_sessions(
    journal_dir: impl AsRef<Path>,
    current_session_id: Option<&str>,
    query: &str,
    limit: usize,
) -> Result<Vec<SessionRecallHit>, SessionRecallError> {
    let journal_dir = journal_dir.as_ref();
    let sessions_root = journal_dir.join(".sessions");
    if !sessions_root.is_dir() {
        return Ok(Vec::new());
    }
    let canonical_root = std::fs::canonicalize(journal_dir).ok();
    let terms = query_terms(query);
    let query_norm = normalize(query);
    let limit = limit.clamp(1, 10);

    let mut metas = Vec::new();
    for entry in std::fs::read_dir(&sessions_root)? {
        let entry = entry?;
        let Ok(file_type) = entry.file_type() else {
            continue;
        };
        if !file_type.is_dir() {
            continue;
        }
        let path = entry.path();
        let session_id = entry.file_name().to_string_lossy().to_string();
        if current_session_id.is_some_and(|current| current == session_id) {
            continue;
        }
        let meta_path = path.join("session.json");
        let Ok(meta) = session_meta(&meta_path) else {
            continue;
        };
        if meta.id != session_id || meta.journals.is_empty() {
            continue;
        }
        let modified_unix_ms = entry
            .metadata()
            .ok()
            .and_then(|metadata| metadata.modified().ok())
            .and_then(|modified| modified.duration_since(UNIX_EPOCH).ok())
            .map(|duration| duration.as_millis())
            .unwrap_or_default();
        metas.push((modified_unix_ms, meta));
    }
    metas.sort_by_key(|entry| std::cmp::Reverse(entry.0));
    metas.truncate(MAX_SESSIONS_SCANNED);

    let mut hits = Vec::new();
    let mut scanned_bytes = 0u64;
    'sessions: for (modified_unix_ms, meta) in metas {
        let title = compact(&meta.title, 180);
        let (title_score, mut terms_matched) =
            score_text(&title, &terms, &query_norm, TITLE_WEIGHT);
        let mut evidence = Vec::new();
        let mut latest_run_unix_ms = 0u128;
        for journal_path in meta.journals.iter().take(MAX_JOURNALS_PER_SESSION) {
            if scanned_bytes >= MAX_TOTAL_JOURNAL_BYTES {
                break 'sessions;
            }
            let Some(journal_path) =
                safe_journal_path(journal_dir, canonical_root.as_deref(), journal_path)
            else {
                continue;
            };
            let Ok(metadata) = std::fs::metadata(&journal_path) else {
                continue;
            };
            if metadata.len() > MAX_JOURNAL_BYTES {
                continue;
            }
            scanned_bytes = scanned_bytes.saturating_add(metadata.len());
            let Ok(events) = read_events(&journal_path) else {
                continue;
            };
            let Ok(replay) = replay_events(&events) else {
                continue;
            };
            let run_id = replay.run_id.to_string();
            let (Some(request), Some(answer)) = (replay.request, replay.answer) else {
                continue;
            };
            // An unrepairable structured answer is a runtime failure message,
            // not a memory. Recalling it would rank the failed question above
            // the session that actually answered it.
            if answer
                .constraints
                .iter()
                .any(|constraint| constraint == "answer repair failed")
            {
                continue;
            }
            let request_text = compact(&request.objective, MAX_REQUEST_CHARS);
            let conclusion_text = compact(&answer.conclusion, MAX_CONCLUSION_CHARS);
            let (request_score, request_terms) =
                score_text(&request_text, &terms, &query_norm, REQUEST_WEIGHT);
            let (conclusion_score, conclusion_terms) =
                score_text(&conclusion_text, &terms, &query_norm, CONCLUSION_WEIGHT);
            let run_score = request_score.saturating_add(conclusion_score);
            if !terms.is_empty() && run_score == 0 {
                continue;
            }
            terms_matched.extend(request_terms);
            terms_matched.extend(conclusion_terms);
            let timestamp_unix_ms = events
                .first()
                .map(|event| event.timestamp_unix_ms)
                .unwrap_or_default();
            latest_run_unix_ms = latest_run_unix_ms.max(timestamp_unix_ms);
            evidence.push(SessionRecallEvidence {
                run_id,
                timestamp_unix_ms,
                request: request_text,
                conclusion: conclusion_text,
                status: crate::answer_render::answer_status_label(answer.status).to_string(),
                score: run_score,
            });
        }
        evidence.sort_by(|left, right| {
            right
                .score
                .cmp(&left.score)
                .then_with(|| right.timestamp_unix_ms.cmp(&left.timestamp_unix_ms))
        });
        evidence.truncate(MAX_EVIDENCE_PER_SESSION);
        let best_run_score = evidence.first().map(|item| item.score).unwrap_or(0);
        let score = title_score.saturating_add(best_run_score);
        let mut matched_terms = terms_matched.into_iter().collect::<Vec<_>>();
        matched_terms.sort();
        matched_terms.dedup();
        if !terms.is_empty() {
            let distinctive = matched_terms.iter().any(|term| {
                terms.cjk.contains(term)
                    || (terms.ascii.contains(term) && term.chars().count() >= 2)
                    || term.chars().count() >= 4
            });
            let threshold = if distinctive {
                MIN_DISTINCTIVE_SCORE
            } else {
                MIN_SINGLE_LETTER_SCORE
            };
            if score < threshold {
                continue;
            }
        }
        if evidence.is_empty() {
            continue;
        }
        matched_terms.truncate(8);
        let last_used_unix_ms = if latest_run_unix_ms > 0 {
            latest_run_unix_ms
        } else {
            modified_unix_ms
        };
        hits.push(SessionRecallHit {
            session_id: meta.id,
            title,
            last_used_unix_ms,
            runs: meta.journals.len(),
            score,
            matched_terms,
            evidence,
        });
    }
    hits.sort_by(|left, right| {
        right
            .score
            .cmp(&left.score)
            .then_with(|| right.last_used_unix_ms.cmp(&left.last_used_unix_ms))
    });
    hits.truncate(limit);
    Ok(hits)
}

/// Render recall hits as bounded CONVERSATION CONTEXT facts.
pub fn prompt_facts(
    hits: &[SessionRecallHit],
    query: &str,
    now_unix_ms: u128,
    max_chars: usize,
) -> Vec<String> {
    let query = compact(query, 160);
    if hits.is_empty() {
        return vec![format!(
            "Cross-session memory search for `{query}` found no matching local session record. \
             If the user asks whether you remember, say plainly that you have no record and ask \
             for a topic or time; do not invent one."
        )];
    }
    let mut facts = vec![format!(
        "Cross-session memory search for `{query}` found {} local session record(s). These are \
         retrieved session records, not model memory: answer only from the snippets, cite the \
         session title and earlier request or conclusion, and say so if they do not answer the \
         question. A record is evidence of what was said, not proof that it is still true.",
        hits.len()
    )];
    let mut used = facts[0].chars().count();
    for hit in hits {
        let Some(evidence) = hit.evidence.first() else {
            continue;
        };
        let block = format!(
            "- {} · session `{}` · {} ({} run(s))\n  Earlier user request: {}\n  Earlier answer conclusion: {}\n  Status: {}",
            age_label(hit.last_used_unix_ms, now_unix_ms),
            hit.session_id,
            hit.title,
            hit.runs,
            evidence.request,
            evidence.conclusion,
            evidence.status
        );
        let length = block.chars().count();
        if used.saturating_add(length) > max_chars {
            break;
        }
        used = used.saturating_add(length);
        facts.push(block);
    }
    facts
}

/// Human-readable age used in prompt facts and tool output.
pub fn age_label(timestamp_unix_ms: u128, now_unix_ms: u128) -> String {
    let delta = now_unix_ms.saturating_sub(timestamp_unix_ms);
    const MINUTE: u128 = 60_000;
    const HOUR: u128 = 3_600_000;
    const DAY: u128 = 86_400_000;
    if delta < MINUTE {
        "just now".into()
    } else if delta < HOUR {
        format!("{}m ago", delta / MINUTE)
    } else if delta < DAY {
        format!("{}h ago", delta / HOUR)
    } else if delta < 2 * DAY {
        "yesterday".into()
    } else if delta < 30 * DAY {
        format!("{}d ago", delta / DAY)
    } else {
        format!("{}mo ago", delta / (30 * DAY))
    }
}

fn session_meta(path: &Path) -> Result<SessionMeta, ()> {
    let metadata = std::fs::symlink_metadata(path).map_err(|_| ())?;
    if !metadata.file_type().is_file() || metadata.len() > MAX_SESSION_FILE_BYTES {
        return Err(());
    }
    let bytes = std::fs::read(path).map_err(|_| ())?;
    serde_json::from_slice(&bytes).map_err(|_| ())
}

fn safe_journal_path(
    journal_dir: &Path,
    canonical_root: Option<&Path>,
    journal_path: &Path,
) -> Option<PathBuf> {
    let path = if journal_path.is_absolute() {
        journal_path.to_path_buf()
    } else {
        journal_dir.join(journal_path)
    };
    let canonical = std::fs::canonicalize(path).ok()?;
    if let Some(root) = canonical_root
        && !canonical.starts_with(root)
    {
        return None;
    }
    Some(canonical)
}

fn query_terms(query: &str) -> QueryTerms {
    let mut terms = QueryTerms::default();
    let mut ascii = String::new();
    let mut cjk = String::new();
    for ch in query.chars() {
        if ch.is_ascii_alphanumeric() {
            flush_cjk(&mut cjk, &mut terms);
            ascii.push(ch.to_ascii_lowercase());
        } else if is_cjk(ch) {
            flush_ascii(&mut ascii, &mut terms);
            cjk.push(ch);
        } else {
            flush_ascii(&mut ascii, &mut terms);
            flush_cjk(&mut cjk, &mut terms);
        }
    }
    flush_ascii(&mut ascii, &mut terms);
    flush_cjk(&mut cjk, &mut terms);
    terms
}

fn flush_ascii(word: &mut String, terms: &mut QueryTerms) {
    if word.is_empty() {
        return;
    }
    let word = std::mem::take(word);
    if word.chars().count() <= 40 && !is_ascii_stopword(&word) {
        terms.ascii.insert(word);
    }
}

fn flush_cjk(run: &mut String, terms: &mut QueryTerms) {
    if run.is_empty() {
        return;
    }
    let run = std::mem::take(run);
    let chars = run.chars().collect::<Vec<_>>();
    if chars.len() == 1 {
        return;
    }
    for pair in chars.windows(2) {
        let bigram = pair.iter().collect::<String>();
        if !is_cjk_stopword(&bigram) {
            terms.cjk.insert(bigram);
        }
    }
}

fn score_text(
    text: &str,
    terms: &QueryTerms,
    query_norm: &str,
    weight: u32,
) -> (u32, BTreeSet<String>) {
    if terms.is_empty() {
        return (0, BTreeSet::new());
    }
    let (ascii_words, cjk_bigrams) = text_terms(text);
    let mut score = 0u32;
    let mut matched = BTreeSet::new();
    for term in &terms.ascii {
        if ascii_words.contains(term) {
            score = score.saturating_add(ascii_term_points(term).saturating_mul(weight));
            matched.insert(term.clone());
        }
    }
    for term in &terms.cjk {
        if cjk_bigrams.contains(term) {
            score = score.saturating_add(3u32.saturating_mul(weight));
            matched.insert(term.clone());
        }
    }
    let normalized = normalize(text);
    if query_norm.chars().count() >= 4 && normalized.contains(query_norm) {
        score = score.saturating_add(FULL_PHRASE_BONUS.saturating_mul(weight));
        matched.insert(query_norm.to_string());
    }
    (score, matched)
}

fn text_terms(text: &str) -> (BTreeSet<String>, BTreeSet<String>) {
    let mut ascii = BTreeSet::new();
    let mut cjk = BTreeSet::new();
    let mut ascii_word = String::new();
    let mut cjk_run = String::new();
    for ch in text.chars() {
        if ch.is_ascii_alphanumeric() {
            flush_cjk_bigrams(&mut cjk_run, &mut cjk);
            ascii_word.push(ch.to_ascii_lowercase());
        } else if is_cjk(ch) {
            flush_ascii_word(&mut ascii_word, &mut ascii);
            cjk_run.push(ch);
        } else {
            flush_ascii_word(&mut ascii_word, &mut ascii);
            flush_cjk_bigrams(&mut cjk_run, &mut cjk);
        }
    }
    flush_ascii_word(&mut ascii_word, &mut ascii);
    flush_cjk_bigrams(&mut cjk_run, &mut cjk);
    (ascii, cjk)
}

fn flush_ascii_word(word: &mut String, set: &mut BTreeSet<String>) {
    if !word.is_empty() {
        let word = std::mem::take(word);
        if word.chars().count() <= 80 {
            set.insert(word);
        }
    }
}

fn flush_cjk_bigrams(run: &mut String, set: &mut BTreeSet<String>) {
    if run.is_empty() {
        return;
    }
    let chars = std::mem::take(run).chars().collect::<Vec<_>>();
    for pair in chars.windows(2) {
        set.insert(pair.iter().collect());
    }
}

fn ascii_term_points(term: &str) -> u32 {
    if term.chars().count() == 1 { 2 } else { 3 }
}

fn normalize(text: &str) -> String {
    text.chars()
        .filter(|ch| ch.is_alphanumeric())
        .flat_map(char::to_lowercase)
        .collect()
}

fn compact(text: &str, max_chars: usize) -> String {
    let mut output = String::new();
    let mut count = 0usize;
    let mut pending_space = false;
    for ch in text.chars() {
        if ch.is_control() || ch.is_whitespace() {
            pending_space = !output.is_empty();
            continue;
        }
        if pending_space {
            output.push(' ');
            count += 1;
            pending_space = false;
        }
        if count >= max_chars {
            output.push('…');
            break;
        }
        output.push(ch);
        count += 1;
    }
    output.trim().to_string()
}

fn is_cjk(ch: char) -> bool {
    matches!(
        ch as u32,
        0x3400..=0x4DBF | 0x4E00..=0x9FFF | 0xF900..=0xFAFF
    )
}

fn is_ascii_stopword(word: &str) -> bool {
    matches!(
        word,
        "a" | "an"
            | "and"
            | "are"
            | "as"
            | "at"
            | "be"
            | "before"
            | "by"
            | "chat"
            | "conversation"
            | "did"
            | "discuss"
            | "discussed"
            | "do"
            | "does"
            | "earlier"
            | "for"
            | "from"
            | "had"
            | "has"
            | "have"
            | "how"
            | "i"
            | "in"
            | "is"
            | "it"
            | "last"
            | "me"
            | "memory"
            | "my"
            | "no"
            | "not"
            | "of"
            | "on"
            | "or"
            | "our"
            | "previous"
            | "recall"
            | "remember"
            | "said"
            | "say"
            | "talk"
            | "talked"
            | "that"
            | "the"
            | "this"
            | "time"
            | "to"
            | "us"
            | "was"
            | "we"
            | "were"
            | "what"
            | "when"
            | "where"
            | "which"
            | "who"
            | "why"
            | "yes"
            | "yesterday"
            | "you"
            | "your"
    )
}

fn is_cjk_stopword(word: &str) -> bool {
    matches!(
        word,
        "我们"
            | "你们"
            | "他们"
            | "昨天"
            | "今天"
            | "明天"
            | "上次"
            | "这次"
            | "那个"
            | "这个"
            | "什么"
            | "怎么"
            | "可以"
            | "还有"
            | "印象"
            | "记得"
            | "一下"
            | "说的"
            | "聊的"
            | "聊过"
            | "之前"
            | "以前"
            | "还记"
            | "得吗"
            | "有印"
            | "象吗"
            | "你还"
            | "我聊"
            | "天我"
            | "们聊"
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::SystemTime;

    use crate::domain::{AgentAnswer, AnswerStatus, CognitiveMode, TaskRequest};
    use crate::journal::{Journal, JournalEventKind};
    use uuid::Uuid;

    fn answer(conclusion: &str) -> AgentAnswer {
        AgentAnswer {
            schema_version: crate::domain::ANSWER_SCHEMA_VERSION,
            status: AnswerStatus::Completed,
            conclusion: conclusion.into(),
            claims: Vec::new(),
            evidence: Vec::new(),
            assumptions: Vec::new(),
            constraints: Vec::new(),
            unknowns: Vec::new(),
            conflicts: Vec::new(),
            flip_conditions: Vec::new(),
            action_receipts: Vec::new(),
            task_need: None,
        }
    }

    fn write_run_with_answer(root: &Path, request: &str, answer: AgentAnswer) -> PathBuf {
        let path = root.join(format!("{}.jsonl", Uuid::new_v4()));
        let mut journal = Journal::create(&path, Uuid::new_v4()).unwrap();
        journal
            .append(JournalEventKind::RunStarted {
                request: TaskRequest::from_objective(request, root.to_path_buf()),
                mode: CognitiveMode::DirectAnswer,
            })
            .unwrap();
        journal
            .append(JournalEventKind::AnswerProduced { answer })
            .unwrap();
        journal
            .append(JournalEventKind::RunStopped {
                reason: crate::domain::StopReason::Completed,
            })
            .unwrap();
        path
    }

    fn write_run(root: &Path, request: &str, conclusion: &str) -> PathBuf {
        write_run_with_answer(root, request, answer(conclusion))
    }

    fn session_with_journals(root: &Path, id: &str, title: &str, journals: Vec<PathBuf>) {
        let session_root = root.join(".sessions").join(id);
        std::fs::create_dir_all(&session_root).unwrap();
        let record = serde_json::json!({
            "version": 2,
            "id": id,
            "title": title,
            "journals": journals,
            "archived": false,
            "mentions": [],
        });
        std::fs::write(
            session_root.join("session.json"),
            serde_json::to_vec(&record).unwrap(),
        )
        .unwrap();
    }

    fn session(root: &Path, id: &str, title: &str, runs: &[(&str, &str)]) {
        let journals = runs
            .iter()
            .map(|(request, conclusion)| write_run(root, request, conclusion))
            .collect::<Vec<_>>();
        session_with_journals(root, id, title, journals);
    }

    #[test]
    fn recalls_the_f_g_session_from_a_yesterday_question() {
        let directory = tempfile::tempdir().unwrap();
        let root = directory.path();
        session(
            root,
            "ses_fg",
            "具备“F”和“G” 的 正向的，畅想的，未来感的，科技感的，正义感的，英文单词都有哪些？",
            &[(
                "要F开头的，G再中间的",
                "全部以 F 开头，G 都在词中间：flagship、foreground、forging、fulgent、foregleam",
            )],
        );
        session(
            root,
            "ses_site",
            "给我做个网站",
            &[("帮我做个网站", "已生成云笺页面")],
        );
        let hits = recall_sessions(
            root,
            Some("ses_current"),
            "昨天我们聊的 F开头和G开头你还有印象吗？",
            3,
        )
        .unwrap();
        assert_eq!(hits.len(), 1, "{hits:?}");
        assert_eq!(hits[0].session_id, "ses_fg");
        assert!(hits[0].title.contains("F"));
        assert!(
            hits[0].evidence[0].conclusion.contains("flagship"),
            "{:?}",
            hits[0].evidence
        );
        assert!(
            hits[0]
                .matched_terms
                .iter()
                .any(|term| term == "开头" || term == "f" || term == "g"),
            "{:?}",
            hits[0].matched_terms
        );
        let word_hits = recall_sessions(root, None, "flagship", 3).unwrap();
        assert_eq!(word_hits[0].session_id, "ses_fg", "{word_hits:?}");
    }

    #[test]
    fn unrepairable_runtime_answers_are_not_recalled() {
        let directory = tempfile::tempdir().unwrap();
        let root = directory.path();
        let mut failed = answer("模型返回的答案结构无法修复，已降级为需要证据");
        failed.status = AnswerStatus::NeedsEvidence;
        failed.constraints = vec!["answer repair failed".into()];
        let journal = write_run_with_answer(root, "F开头的单词", failed);
        session_with_journals(root, "ses_failed", "F 和 G 开头的单词", vec![journal]);
        let hits = recall_sessions(root, None, "F开头的单词", 3).unwrap();
        assert!(hits.is_empty(), "{hits:?}");
    }

    #[test]
    fn excludes_the_current_session_and_unknown_queries() {
        let directory = tempfile::tempdir().unwrap();
        let root = directory.path();
        session(
            root,
            "ses_fg",
            "F 和 G 开头的单词",
            &[("F开头的单词", "flagship")],
        );
        let hits = recall_sessions(root, Some("ses_fg"), "F开头的单词", 5).unwrap();
        assert!(hits.is_empty(), "{hits:?}");
        let hits = recall_sessions(root, None, "完全无关的火星天气", 5).unwrap();
        assert!(hits.is_empty(), "{hits:?}");
    }

    #[test]
    fn an_empty_or_topicless_query_lists_recent_sessions() {
        let directory = tempfile::tempdir().unwrap();
        let root = directory.path();
        session(
            root,
            "ses_one",
            "第一个会话",
            &[("部署测试环境", "环境已就绪")],
        );
        session(
            root,
            "ses_two",
            "第二个会话",
            &[("检查发布包", "发布包校验通过")],
        );
        for query in ["", "你还记得吗？"] {
            let hits = recall_sessions(root, None, query, 2).unwrap();
            assert_eq!(hits.len(), 2, "{query}: {hits:?}");
            assert!(hits.iter().all(|hit| !hit.evidence.is_empty()));
        }
    }

    #[test]
    fn prompt_facts_are_bounded_and_name_the_no_record_case() {
        let directory = tempfile::tempdir().unwrap();
        let root = directory.path();
        session(
            root,
            "ses_fg",
            "F 和 G 开头的单词",
            &[(
                "要F开头的，G再中间的",
                "flagship、foreground、forging、fulgent、foregleam",
            )],
        );
        let hits = recall_sessions(root, None, "F开头和G开头", 3).unwrap();
        let facts = prompt_facts(&hits, "F开头和G开头", 1_700_000_000_000, 4_000);
        let text = facts.join("\n");
        assert!(text.contains("ses_fg"), "{text}");
        assert!(text.contains("Earlier answer conclusion"), "{text}");
        assert!(text.chars().count() <= 4_000, "{}", text.chars().count());

        let empty = prompt_facts(&[], "你还记得吗", 1_700_000_000_000, 4_000);
        assert!(
            empty
                .join("")
                .contains("found no matching local session record")
        );
        assert!(empty.join("").contains("do not invent"));
    }

    #[test]
    fn uses_run_time_not_directory_mtime_for_age() {
        let directory = tempfile::tempdir().unwrap();
        let root = directory.path();
        session(
            root,
            "ses_old",
            "F 和 G 开头的单词",
            &[("F开头的单词", "flagship")],
        );
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_millis();
        let old = now.saturating_sub(22 * 3_600_000);
        let session_root = root.join(".sessions").join("ses_old");
        let record: serde_json::Value =
            serde_json::from_slice(&std::fs::read(session_root.join("session.json")).unwrap())
                .unwrap();
        let journal_path = record["journals"][0].as_str().unwrap();
        let lines = std::fs::read_to_string(journal_path).unwrap();
        let mut rewritten = String::new();
        for line in lines.lines() {
            let mut event: serde_json::Value = serde_json::from_str(line).unwrap();
            event["timestamp_unix_ms"] = serde_json::json!(old);
            rewritten.push_str(&serde_json::to_string(&event).unwrap());
            rewritten.push('\n');
        }
        std::fs::write(journal_path, rewritten).unwrap();

        let hits = recall_sessions(root, None, "F开头的单词", 1).unwrap();
        assert_eq!(hits.len(), 1, "{hits:?}");
        assert_eq!(hits[0].last_used_unix_ms, old);
        assert_eq!(age_label(hits[0].last_used_unix_ms, now), "22h ago");
    }

    #[test]
    fn age_labels_are_stable() {
        let now = 1_700_000_000_000u128;
        assert_eq!(age_label(now, now), "just now");
        assert_eq!(age_label(now - 5 * 60_000, now), "5m ago");
        assert_eq!(age_label(now - 3 * 3_600_000, now), "3h ago");
        assert_eq!(age_label(now - 30 * 3_600_000, now), "yesterday");
        assert_eq!(age_label(now - 3 * 86_400_000, now), "3d ago");
    }
}
