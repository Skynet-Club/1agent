# ADR 0081: File Chat Bridge

- Status: accepted
- Date: 2026-11-12

## Context

The chat adapter contract maps `ChatMessage` to `TaskRequest` and
`AgentAnswer` to `ChatReply`, but there was no executable adapter surface.
SA98-51 asks for a presence entry. A real Slack/Telegram/WeChat transport needs
credentials, reply routing and idempotency; a file-based bridge is the smallest
executable step and can be used by a local worker or channel integration.

## Decision

1. Add `src/chat_bridge.rs`:
   - `task` reads a `ChatMessage` JSON, validates non-empty sender/thread/text,
     maps it with `chat::to_task_request` and writes a `TaskRequest` JSON;
   - `reply` reads a `RunResult` JSON, maps `answer` with
     `chat::format_reply` and writes a `ChatReply` JSON.
2. Inputs are bounded to 1 MiB and must be regular non-symlink files. Writes
   create the parent directory, use a temporary file and rename.
3. `ingest` scans up to 500 top-level `*.json` messages, writes
   `<stem>.task.json`, moves valid messages to `processed/` and invalid ones to
   `failed/` with `<stem>.error.txt`. If `<stem>.task.json` already exists, the
   task is not overwritten; the message moves to `processed/` and counts as
   skipped. With `--state PATH`, the SHA-256 of each converted/skipped message
   is stored (max 10,000) and the same content under another file name is
   moved to `processed/` as a duplicate. A failed message makes the command
   exit non-zero after reporting all results.
4. `1agent chat-bridge task|reply|ingest` exposes the three operations.
5. The bridge is not a daemon and does not connect to Slack/Telegram/WeChat.
   It does not execute tasks; `ingest` only moves files into processed/failed.

## Consequences

- A local integration can turn a channel message file into a runnable task and
  turn a run result into a channel reply without parsing schemas itself.
- The files are plaintext and may contain sensitive chat text; filesystem
  permissions and retention are the operator's responsibility.
- `ingest` is a single-process spool, not a durable queue. It has no retry,
  locking or acknowledgement; `--state` provides content dedupe up to 10,000
  hashes, then fails closed rather than growing without bound.
- A task file can be written before the source move fails; the next run would
  see the same stem and skip it, so the task is not overwritten.
- State is saved after processing; if saving fails, already-moved messages
  are not rolled back.
- The output `ChatReply` includes the conclusion, one trust line and the
  approval/inbox flags; it does not leak schema field names.

## Evidence

```bash
cargo test chat_bridge --lib
cargo test --test chat_bridge_cli
```

The tests cover message→task conversion, whitespace trimming, empty-text
rejection, and a real deterministic run result→reply conversion with
`thread_id` and `inbox_open`.
