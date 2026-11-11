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
6. This is a pure policy. Sending the escalation, changing the item status,
   stopping after acknowledgement and per-channel ladders are separate.

## Consequences

- A single ordered definition covers reminders and urgent escalation.
- Strictly increasing thresholds prevent ambiguous ties.
- The ladder cannot know whether an item was already delivered on another
  channel; the caller must pass a resolved/acknowledged flag.
- The action names are abstract. Mapping them to chat, email, phone or a
  desktop notification is a transport concern.
- No persistence; each call recomputes from the item age.

## Evidence

```bash
cargo test escalation --lib
cargo test --test escalate_cli
cargo test --test inbox_escalate_cli
```

The tests cover each rung, resolution, a grace period, invalid/empty ladders,
CLI parsing of repeated steps, and the inbox projection over open versus
resolved items.
