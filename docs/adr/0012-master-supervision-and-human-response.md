# ADR 0012: Master Supervision and Human Response

Status: accepted

Date: 2026-11-03

## Context

Dispatching a Workshop Session is not sufficient for production control. A Master
operator must be able to see the board, poll bounded progress signals, identify tasks
that require attention, route an answer to the correct Session and issue explicit
control decisions without bypassing the Master state machine.

These operations are Workshop Master commands. They remain invisible and rejected in
standalone 1A and Worker mode.

## Decision

Add six Master-only commands:

- `/board [filter]` renders the Work Item board from durable `MasterState`, including
  title, status, dependencies, Agent, Session, lease expiry, generation and last error.
- `/watch` polls active Workshop Sessions using the bounded `[1A:RUNNING]`,
  `[1A:AWAITING_INPUT]` and `[1A:BLOCKED]` protocol, applies observations to
  `MasterState`, and appends journal events for state transitions or terminal receipts.
- `/attention` lists `AwaitingInput`, `ArtifactReady`, `Blocked`, `Rejected`, `Retrying`
  and `Replanning` tasks, plus active leases that have expired.
- `/respond <task-id> <message>` is valid only for `AwaitingInput` or `Blocked` tasks. It
  writes to the bound Workshop Session, moves the task to `Running`, and journals the
  operator response as an `Observed` event.
- `/control pause|resume|cancel|takeover|return|retry|replan|reclaim` records explicit
  operator decisions. Destructive Session cancellation occurs only after the requested
  task transition and bounded retry/replan preconditions have been validated. Reclaim
  requires an expired lease and confirmation that the old execution is terminated.
  Return ends a human takeover and emits `human_takeover.ended`.
- `/timeline` renders the bounded tail of the Master journal and Workshop event stream.
  Workshop events are annotated with a matching Session ID when the event payload
  contains one, or `session=unassociated` when it does not.

`/watch` does not ingest raw logs or duplicate Workshop scheduling. Workshop remains
authoritative for Session lifecycle and process control. `takeover` marks the Work Item
as human-controlled so automatic polling no longer advances it; the operator can still
inspect or explicitly cancel the Session.

## Consequences

- Human input is routed to an identified Session instead of being inserted into a
  separate chat context.
- Task state changes remain durable, replayable and auditable.
- Invalid cancel, retry or replan requests fail before terminating a live Session.
- `/attention` exposes both explicit operator questions and completed Artifacts awaiting
  independent verification.
- Timeline output is bounded and correlates Workshop events only by observed Session
  identifiers; it does not claim a causal relation that the payload does not contain.

## Verification

- `AwaitingInput -> Running` and `Blocked -> Running` transitions survive journal replay
  and audit;
- `/watch` consumes fake Workshop progress markers and updates Master state;
- `/respond` reaches a fake Workshop Session and records the response;
- pause, resume, cancel, takeover, retry and replan are journaled, with retry/replan
  limits enforced;
- `/board`, `/attention` and `/timeline` have deterministic rendering tests;
- all six commands are unavailable in standalone and Worker mode;
- a CLI E2E run exercises board, watch, attention, respond, controls, timeline and final
  cancellation through the real TUI command dispatcher.
