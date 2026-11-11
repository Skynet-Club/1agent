# One-Way Inbox Notification — Evidence (2026-11-12)

Status: **passed on this machine**  
Scope: `notify::InboxNotifier` and `1agent inbox-notify`.

## Commands

```bash
cargo test notify --lib
cargo test --test inbox_notify_cli
cargo clippy --all-targets -- -D warnings
```

## Expected results

```text
running 4 tests (notify module)
test notify::tests::insecure_remote_and_embedded_credentials_are_rejected ... ok
test notify::tests::payloads_are_bounded_and_formatted ... ok
test notify::tests::watermark_suppresses_unchanged_items_and_survives_save ... ok
test notify::tests::receipts_append_as_jsonl ... ok

test result: ok. 4 passed; 0 failed

running 2 tests (tests/inbox_notify_cli.rs)
test inbox_notify_posts_open_items_to_a_slack_webhook ... ok
test inbox_notify_skips_empty_queues_and_rejects_insecure_urls ... ok

test result: ok. 2 passed; 0 failed
```

## What this proves

- A loopback Slack webhook receives `{text}` with the open count and both item
  titles.
- An empty queue returns `sent: false` and sends no request.
- Remote plain HTTP and URLs with embedded credentials are rejected.
- Generic payload titles are bounded to 200 characters plus an ellipsis.
- `--state` suppresses a repeat with no newer item, survives save/reopen, and
  does not advance when a send fails; a newly added item triggers another send
  with `new_items: 1`.
- `--log` appends one receipt per successful send and no receipt for a
  suppressed repeat; the receipt records format and counts but not the URL.

## What this does not prove

- Real Slack/Discord delivery, authentication or rate limits.
- Reply handling, item mutation or a full chat transport.
- Retry, delivery receipt or deduplication.
- That webhook URLs are handled as secrets by the shell/caller.
- Cross-platform behavior; tests ran on Linux only.

## Files

- `src/notify.rs`
- `src/main.rs` (`inbox-notify`)
- `tests/inbox_notify_cli.rs`
- `docs/adr/0080-one-way-inbox-notification.md`
- `docs/architecture.md`, `docs/features.md`, `docs/features.en.md`,
  `docs/claims.md`, `CHANGELOG.md`
