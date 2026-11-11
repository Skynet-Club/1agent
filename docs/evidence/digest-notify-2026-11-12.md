# Digest Webhook Notification — Evidence (2026-11-12)

Status: **passed on this machine**  
Scope: `digest --notify-webhook` and the shared one-way notifier text path.

## Commands

```bash
cargo test --test digest_notify_cli
cargo clippy --all-targets -- -D warnings
```

## Expected results

```text
running 2 tests (tests/digest_notify_cli.rs)
test digest_posts_its_text_summary ... ok
test digest_rejects_an_insecure_webhook ... ok

test result: ok. 2 passed; 0 failed
```

## What this proves

- A loopback Slack webhook receives the text digest with the header and inbox
  count.
- The JSON digest is still emitted on stdout.
- A remote plain-HTTP webhook is rejected before sending.

## What this does not prove

- Real Slack/Discord delivery, auth or rate limits.
- Scheduling of the digest or retry/deduplication.
- That the webhook URL is treated as a secret by the caller.
- Cross-platform behavior; tests ran on Linux only.

## Files

- `src/main.rs` (`digest --notify-webhook/--notify-format`)
- `src/notify.rs` (`send_text`)
- `tests/digest_notify_cli.rs`
- `docs/adr/0075-digest-attention-summary.md`
- `docs/architecture.md`, `docs/features.md`, `docs/features.en.md`,
  `docs/claims.md`, `CHANGELOG.md`
