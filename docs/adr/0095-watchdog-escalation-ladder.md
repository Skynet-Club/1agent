# ADR 0095: Watchdog Escalation Ladder

- Status: accepted
- Date: 2026-11-12

## Context

The out-of-band watchdog only reported `healthy` or `unhealthy` from a
heartbeat age. Operators had no explicit escalation policy and no durable
record of when the watchdog recommended pausing or repairing the daemon. The
watchdog deliberately has no shared memory, socket or process authority, and
this decision must not give it any.

## Decision

1. Add `WatchdogPolicy` with three strictly increasing thresholds:
   `warn_after_ms`, `pause_after_ms` and `repair_after_ms`. Invalid order or a
   zero warn threshold is rejected.
2. `decide` maps a fresh heartbeat to `ok`, and a stale one to `warn`,
   `pause` or `repair` by threshold. A missing heartbeat goes straight to
   `repair` because there is nothing to observe.
3. `pause` and `repair` are recommendations. The watchdog never signals a
   process, never restarts the daemon and never changes permissions. Executing
   an action remains an operator decision.
4. `append_intervention` writes a JSONL record only when the action changes;
   repeating the same action is not logged again, and a recovery to `ok` is
   logged once so the trail shows both escalation and recovery.
5. The CLI keeps the existing `--max-age-ms` status output when no policy is
   configured. With all three thresholds it prints a `WatchdogDecision`
   (`action`, `reason`, `pid`, `age_ms`) and still exits non-zero for any
   action other than `ok`. `--intervention-log` is optional and never needs
   write authority over the daemon.

## Consequences

- Operators get a deterministic ladder and an append-only transition log
  instead of a binary healthy/unhealthy signal.
- The watchdog remains out-of-band and read-only with respect to the daemon,
  so a compromised or wedged watchdog cannot stop or restart work.
- Actual pause/repair execution is not implemented; this slice stops at
  policy, recommendation and audit trail.
- The log is local JSONL without rotation; the caller controls invocation
  cadence and archive policy.
