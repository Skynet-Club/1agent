# ADR 0057: Chat Adapter Contract

- Status: accepted
- Date: 2026-11-12

## Context

The partner needs a presence surface outside the TUI: Slack, Telegram, WeChat
or a web chat. Implementing a transport first would bake channel-specific
assumptions into the runtime. The mapping should be defined and tested without
a network.

## Decision

1. Add `src/chat.rs` with:
   - `ChatChannel` (slack/telegram/wechat/web/other);
   - `ChatMessage { channel, sender_id, thread_id, text }`;
   - `ChatReply { thread_id, text, needs_approval, inbox_open }`.
2. `to_task_request(message, workspace)` maps the trimmed text to a
   `TaskRequest`; the caller validates and transports it.
3. `format_reply(answer, thread_id, inbox_open)` returns:
   - the conclusion first;
   - one trust line (`证据 N · 未知 M`);
   - an approval line and `needs_approval=true` for
     `NeedsAuthorization`;
   - `inbox_open` so the chat can show the queue count.
4. The reply never includes schema field names, raw claims, or action-receipt
   internals.
5. This slice does not implement a Slack/Telegram/WeChat transport, auth,
   threading or retries.

## Consequences

- A transport can be added without changing the runtime answer contract.
- The chat reply stays concise and keeps the trust summary.
- The `inbox_open` count lets a chat surface point at the inbox.
- A real channel still needs rate limits, auth, idempotency and delivery
  receipts.

## Evidence

```bash
cargo test chat --lib
```

The tests cover message-to-task mapping, conclusion/trust formatting without
schema leaks, and the approval flag.
