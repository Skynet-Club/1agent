# ADR 0064: Escalation Ladder

- Status: accepted
- Date: 2026-11-12

## Context

Inbox items can be snoozed and quiet hours can defer proactive work, but an
unresolved item has no notion of getting louder over time. SA98-52 calls for an
escalation ladder so a forgotten approval does not stay silent forever.

## Decision

1. Add `EscalationStep { after_ms, action }` with actions `inbox`, `digest`,
   `notify`, `urgent`.
2. `EscalationLadder::new` requires at least one step and strictly increasing
   `after_ms` thresholds.
3. `plan(ladder, created_ms, now_ms, resolved)`:
   - returns no level when the item is resolved;
   - returns no level before the first threshold (grace period);
   - otherwise returns the highest step whose `after_ms` the item has reached.
4. `1agent escalate` takes repeated `--step AFTER_MS=ACTION`, `--created-ms`,
   optional `--now-ms` (defaults to now) and `--resolved`. Bad thresholds or
   actions exit non-zero.
5. `1agent inbox escalate` applies the same ladder to open and due-snoozed
   inbox items using each item's `created_ms`; terminal items are excluded.
6. With `--webhook URL --state PATH`, the command delivers a batch of items
   whose level is newly `notify` or `urgent` (higher than the highest rung
   recorded for that item), at most 50 per invocation. Delivery uses the
   existing no-redirect notifier. The state advances only after a successful
   send, so a crash in between can duplicate one batch. Resolved items are
   pruned from the state. `--dry-run` reports without sending.
7. `daemon --escalate-inbox-store PATH --escalate-step ... --escalate-webhook
   URL --escalate-state PATH [--escalate-format ...]` runs the same projection
   and delivery on every tick using one shared implementation. Delivery
   failures are logged and retried on the next tick; quiet hours suppress
   delivery because the daemon takes the quiet branch before it.
8. Delivery does not change item status or stop after acknowledgement.
   `digest --escalation-step` counts every rung, including `digest`, but
   `digest`-rung delivery and per-channel ladders remain separate.

## Consequences

- A single ordered definition covers reminders and urgent escalation.
- Strictly increasing thresholds prevent ambiguous ties.
- The ladder cannot know whether an item was already delivered on another
  channel; the caller must pass a resolved/acknowledged flag.
- The action names are abstract. Mapping them to chat, email, phone or a
  desktop notification is a transport concern.
- Delivery state persists only the highest delivered rung per item, not the
  item itself; each call recomputes the level from the item age.

## Evidence

```bash
cargo test escalation --lib
cargo test --test escalate_cli
cargo test --test inbox_escalate_cli
cargo test --test daemon_escalation_cli
```

The tests cover each rung, resolution, a grace period, invalid/empty ladders,
CLI parsing of repeated steps, the inbox projection over open versus resolved
items, one delivery per reached rung including a raised level, and the daemon
path including quiet-hours suppression.
