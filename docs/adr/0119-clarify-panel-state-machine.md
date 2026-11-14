# ADR 0119: Clarify Batch Panel State Machine

- Status: accepted
- Date: 2026-11-12
- Related: SA97-35

## Context

SA97-34 delivered the clarify gate and round outcome states, but a frontend
still had to invent its own selection, locking, confirmation and timeout
behavior. SA97-35 asks for a blocking panel with one to five questions, one
confirmation, per-question locking and a 300-second timeout. Rendering and
terminal input are separate frontend concerns.

## Decision

1. `ClarifyPanel` owns the state machine: normalized questions, selected
   answers, deadline, and final outcome.
2. Construction requires one to five questions, at least one blocking
   question, valid questions, and unique ids. The recommended option is moved
   first and remains the default when the panel times out.
3. `select` locks one option per question. The automatic `Other` option
   requires non-empty text (≤500 characters, no control characters); other
   options must not carry text. A second selection, skip or select after
   closure fails closed.
4. `skip` records `skipped`. `confirm` submits when at least one question was
   answered and treats an all-skipped confirmation as a cancellation, avoiding
   the invalid empty-submission state.
5. `expire` marks `timed_out` at or after the 300-second deadline. Missing
   questions remain `unanswered`; `default_answer_valid` stays true.
6. `cancel` closes the panel with `cancelled` and preserves unanswered
   defaults. The panel does not render, read a terminal or persist history.

## Consequences

- A TUI or CLI adapter can drive one state machine instead of duplicating
  selection/timeout rules.
- The panel itself does not prove a 300-second real-time block; it is a
  deterministic clock-driven state machine.
- TUI rendering, key handling and persistent question history remain open.
- Free-text `Other` content is stored in the panel answer but not passed into
  `ClarifyAnswer`; a frontend must carry it separately.
