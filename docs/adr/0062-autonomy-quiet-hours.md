# ADR 0062: Autonomy Quiet Hours

- Status: accepted
- Date: 2026-11-12

## Context

The autonomy dial decides whether the partner observes, suggests, acts or asks
for approval, but it has no notion of time. A proactive action at 02:00 is
technically allowed yet socially wrong. SA98-52 requires quiet hours.

## Decision

1. Add `QuietHours { start_minute, end_minute }` and parse `HH:MM-HH:MM`.
   Windows may cross midnight; start is inclusive and end is exclusive.
2. Add `AutonomyDecision::Defer` for work that is allowed but should wait.
3. `decide_proactive` first applies the existing `decide` policy, then:
   - returns it unchanged when quiet hours are unset, the current minute is
     outside the window, the work is user-initiated, or explicit approval was
     given;
   - returns `Defer` with the minutes until the window ends when the base plan
     is `Act` or `RequireApproval` and the work is proactive.
4. `Observe` and `Suggest` are unchanged; quiet hours target execution and
   approval prompts, not analysis.
5. Add `1agent autonomy --quiet HH:MM-HH:MM --now HH:MM [--user-initiated]`.
   Invalid windows or times exit non-zero.
6. This is policy, not a scheduler. Persisting the quiet window, waking at the
   end of it, and re-running deferred work remain separate slices.

## Consequences

- A partner can hold proactive work overnight without user-specific code.
- User-initiated work is never silently delayed.
- Explicit approval is treated as engagement and wins over quiet hours.
- A deferred plan has no stored wake-up; unless a caller persists it, the
  intent is only visible in the decision reason.
- Local time zones and DST are the caller's responsibility; the policy works in
  minutes since midnight.

## Evidence

```bash
cargo test autonomy --lib
cargo test --test autonomy_cli
```

The tests cover cross-midnight and daytime windows, invalid input, deferral of
proactive act/approval, and the user-initiated/explicit-approval exceptions.
