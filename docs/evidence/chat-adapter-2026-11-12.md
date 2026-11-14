# Chat Adapter Contract — Evidence (2026-11-12)

Status: **passed on this machine**  
Scope: `chat::to_task_request` and `chat::format_reply`. This is a
channel-neutral contract; no Slack/Telegram/WeChat transport exists.

## Commands

```bash
cargo test chat --lib
cargo clippy --all-targets -- -D warnings
```

## Expected results

```text
running 3 tests
test chat::tests::message_maps_to_a_task_request ... ok
test chat::tests::reply_keeps_the_conclusion_and_trust_line_without_schema_leaks ... ok
test chat::tests::authorization_marks_the_reply_as_needing_approval ... ok

test result: ok. 3 passed; 0 failed
```

## What this proves

- A chat message maps to a `TaskRequest` with the trimmed text.
- A reply starts with the conclusion and includes one trust line
  (`证据 1 · 未知 1`).
- The reply does not contain `schema_version`, `claims` or `action_receipts`.
- `NeedsAuthorization` sets `needs_approval=true` and adds an approval line.
- `inbox_open` is carried through to the reply.

## What this does not prove

- That a real Slack/Telegram/WeChat transport works; no network transport is
  implemented.
- That channel auth, rate limits, idempotency and delivery receipts are
  handled.
- That users prefer this reply shape; no user study was run.
- Cross-platform behavior; the tests ran on Linux only.

## Files

- `src/chat.rs`
- `docs/adr/0057-chat-adapter-contract.md`
- `docs/architecture.md`, `docs/features.md`, `docs/features.en.md`,
  `docs/claims.md`, `CHANGELOG.md`
