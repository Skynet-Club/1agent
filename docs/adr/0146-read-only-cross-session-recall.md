# ADR 0146: Read-Only Cross-Session Recall

- Status: accepted
- Date: 2026-11-17
- Related: ADR-0002, ADR-0014, ADR-0028, `src/session_recall.rs`, `src/tools/session.rs`

## Context

The TUI already stored every run as a journal and grouped runs into sessions.
It also injected the current session's last six runs into `CONVERSATION
CONTEXT`. A new session started with no facts from any other session. The
layered `MemoryService` existed, but the TUI never wrote conversation records to
it and no runtime tool read it, so it did not provide conversational recall.

A real new session asked `昨天我们聊的 F开头和G开头你还有印象吗？`. The previous
session contained the complete F/G word list, but the model answered `没印象`
because its context held only the current session's website task. That answer
was honest but wrong about the local store.

## Decision

1. Add `session_recall`, a read-only search over the session store. It reads
   session titles plus each journal's user request and answer conclusion. Tool
   observations are parsed while replaying a journal but are never used or
   returned. It does not edit or forget records, and does not promote a
   retrieved answer to verified truth.
2. Exclude the current session id. A conversation cannot recall itself.
3. Use lexical terms (ASCII words, CJK bigrams and a normalized full phrase)
   with field weights and a distinctive-term threshold. There is no embedding
   model and no semantic claim.
4. Before a task whose objective looks like a memory question
   (`昨天`, `上次`, `记得`, `印象`, `聊过`, `remember`, `yesterday`, …), the TUI
   retrieves at most three earlier sessions, bounded to 4,000 characters, and
   appends exact snippets to `CONVERSATION CONTEXT`. The same detector disables
   the one-call chat fast path so tools remain available.
5. Register `session_search` as a low-risk read-only runtime tool. It exposes
   the same search to a full run, excludes the current TUI session, and treats
   an empty query as "list recent sessions".
6. Make no-match an explicit fact. The injected context and the tool output say
   that no local record matched and that the model must say it has no record.
   The system and chat prompts forbid inventing a memory.
7. Keep the existing layered `MemoryService` as a separate curated store. This
   ADR does not auto-write conversations into it, does not create preferences
   or personality facts, and does not change forget/retention semantics.

## Consequences

- A new session can answer a question about an earlier conversation and cite
  the session title, earlier request and earlier conclusion.
- Retrieval is deterministic and inspectable: the selected facts are written to
  the run journal as `known_facts`, so the model-visible input is
  reconstructable.
- Lexical matching can miss a paraphrase that shares no ASCII word or CJK
  bigram. That is a known limit; the model must say no record matched rather
  than guess.
- A previous session that contains the same question and a refusal can also
  match. It is shown as a record, not hidden; the substantive record wins on
  score and the model can acknowledge the earlier miss.
- Archived sessions remain searchable until they are trashed. Trashed sessions
  are outside the scanned directory.
- Session metadata and journal paths are validated under the canonical journal
  root; symlinked `session.json` files and out-of-root journals are ignored.

## Alternatives considered

- **Use the encrypted `MemoryService` directly.** It would require a summarizer,
  scope/consent decisions and automatic ingestion of every answer, and it would
  store model-written summaries rather than exact conversation evidence. Deferred.
- **Embed everything.** A vector index adds a model dependency, opaque ranking
  and a larger privacy surface. Not justified for the first cross-session slice.
- **Inject the last N session titles on every turn.** It misses older sessions
  (the F/G session was 14 sessions back) and spends context on every task.
  Retrieval on a memory question is narrower and cheaper.
