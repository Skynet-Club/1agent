# File Chat Bridge — Evidence (2026-11-12)

Status: **passed on this machine**  
Scope: `chat_bridge::message_to_task`, `chat_bridge::result_to_reply` and
`1agent chat-bridge`.

## Commands

```bash
cargo test chat_bridge --lib
cargo test --test chat_bridge_cli
cargo clippy --all-targets -- -D warnings
```

## Expected results

```text
running 4 tests (chat_bridge module; the lock test is CLI-level)
test chat_bridge::tests::ingest_dir_moves_valid_and_failed_messages ... ok
test chat_bridge::tests::invalid_messages_are_rejected ... ok
test chat_bridge::tests::message_files_become_task_requests ... ok
test chat_bridge::tests::state_deduplicates_the_same_content_under_a_new_name ... ok

test result: ok. 4 passed; 0 failed

running 3 tests (tests/chat_bridge_cli.rs)
test chat_bridge_converts_messages_and_results ... ok
test chat_bridge_ingest_processes_a_spool_directory ... ok
test chat_bridge_rejects_an_invalid_message ... ok

test result: ok. 3 passed; 0 failed
```

## What this proves

- A Slack `ChatMessage` file becomes a `TaskRequest` with the text trimmed and
  the workspace as the read root.
- An empty-text message is rejected.
- A real deterministic run result becomes a `ChatReply` with `thread_id`,
  `inbox_open` and a trust line.
- Writes create the output path and are atomic.
- `ingest` processes 3 messages (2 valid, 1 invalid): converted=2, failed=1,
  valid messages move to processed/, invalid ones to failed/ with an error
  file, and a second run scans 0.
- With a pre-existing `a.task.json`, ingest reports converted=1, skipped=1 and
  leaves the existing task untouched.
- With `--state`, the same message content copied under a new name reports
  duplicates=1 and converted=0; the state survives across runs.
- `--lock` is created for a run and removed on exit; an existing lock makes
  the ingest exit non-zero without touching it.

## What this does not prove

- Slack/Telegram/WeChat connectivity, auth, rate limits or reply routing.
- Task execution, queueing, retry or deduplication.
- File permissions, retention or encryption.
- Cross-platform behavior; tests ran on Linux only.

## Files

- `src/chat_bridge.rs`
- `src/main.rs` (`chat-bridge task|reply`)
- `tests/chat_bridge_cli.rs`
- `docs/adr/0081-file-chat-bridge.md`
- `docs/architecture.md`, `docs/features.md`, `docs/features.en.md`,
  `docs/claims.md`, `CHANGELOG.md`
