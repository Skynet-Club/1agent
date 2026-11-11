# ADR 0063: False-Alarm Budget

- Status: accepted
- Date: 2026-11-12

## Context

The partner can already dedupe attention candidates and defer quiet hours, but
there is no ceiling on how many alerts it may deliver. A noisy partner trains
the user to ignore it. SA98-52 calls for a false-alarm budget.

## Decision

1. Add `AlarmBudget { max_delivered, max_false_alarm_bps, critical_urgency }`
   and `AlarmWindow { delivered, false_alarms }`. The caller owns the window
   (for example, the last 24 hours).
2. `false_alarm_rate_bps` is `false_alarms * 10000 / delivered`, or `None`
   when nothing was delivered. `false_alarms` is capped at `delivered`.
3. `evaluate` returns:
   - `Deliver` for urgency at or above `critical_urgency`, regardless of budget;
   - `Digest` when `delivered >= max_delivered`;
   - `Digest` when the rate exceeds `max_false_alarm_bps`;
   - `Deliver` otherwise.
4. `1agent alarm-budget` exposes the policy and rejects urgency outside
   0..=100 and a false-alarm ceiling outside 0..=10000.
5. This is routing policy only. Sending, persisting, acknowledging and learning
   from user feedback are separate.

## Consequences

- A partner can cap interruptions without hiding safety-critical events.
- Digest is preferred over dropping; information remains reviewable.
- The window and its counts are caller-owned, so a bad caller can defeat the
  budget by passing zeros.
- A critical urgency of 0 makes every alert critical; this is explicit and
  visible rather than a special case.
- The rate is a coarse aggregate; per-category budgets are future work.

## Evidence

```bash
cargo test alarm_budget --lib
cargo test --test alarm_budget_cli
```

The tests cover delivery within budget, exhausted budget, an excessive
false-alarm rate, critical bypass, an empty window, and CLI range validation.
