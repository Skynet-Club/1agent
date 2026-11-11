# ADR 0080: One-Way Inbox Notification

- Status: accepted
- Date: 2026-11-12

## Context

SA98-51 asks for a presence entry (desktop app or a chat bridge). A full
Slack/Telegram/WeChat transport needs credentials, webhooks, reply routing and
idempotency. The smallest useful presence step is to let the partner push its
open inbox queue to a chat webhook.

## Decision

1. Add `InboxNotifier` in `src/notify.rs` with generic/Slack/Discord payloads:
   - generic: `{title, open, items[]}` with item id/kind/title/due;
   - Slack: `{text}`;
   - Discord: `{content}`.
2. URL policy: HTTPS, or HTTP only for loopback; URLs with embedded credentials
   are rejected. The URL is never printed.
3. The payload is bounded to 50 items and 200 title characters per item; the
   request times out after 5 seconds.
4. An empty queue sends nothing and reports `sent: false`.
5. `--state PATH` stores `{last_sent_ms}`. When no item is newer than the
   watermark, nothing is sent. The full open queue is posted when at least one
   item is newer; the watermark advances only after a successful send.
6. `1agent inbox-notify --store PATH --webhook URL [--format ...]
   [--state PATH]` exposes it. It does not read replies, mark items, or store
   the webhook.
7. This is one-way presence. The existing `notify` command still sends one
   operator message; full chat transports remain future work.

## Consequences

- A user can receive the inbox in Slack/Discord via an incoming webhook without
  a new daemon or bot.
- The webhook URL is a secret; it is read from argv in this slice (operators
  should prefer a wrapper/environment when possible).
- There is no retry, delivery receipt, rate limit or deduplication; a failed
  send returns an error and the caller can retry manually.
- The message includes item titles, which may be sensitive; the operator chose
  the webhook destination.
- Notifications are not scheduled; `digest`/cron must invoke the command.

## Evidence

```bash
cargo test notify --lib
cargo test --test inbox_notify_cli
```

The tests cover bounded/plain payloads, insecure URL rejection, a loopback
Slack webhook receiving two items, and an empty queue reporting `sent: false`.
