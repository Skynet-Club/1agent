# ADR 0141: Clarify Bottom Bar Instead of a Full-Screen Modal

- Status: accepted
- Date: 2026-11-16
- Related: ADR-0119, ADR-0123, `docs/tui.md`

## Context

ADR-0123 made the clarify panel a full-screen modal. It owns the keyboard and
clears the whole frame, so a one-question clarification hides the transcript
the user needs to make the decision. In practice, the common case is one
question with two or three options (for example, “帮我搞个网站” needs a
direction, not a configuration form). A full-screen takeover is too heavy for
that case, and it makes the agent feel like it opened a settings screen.

The `ClarifyPanel` state machine already owns validation, per-question locks,
`Other`, confirmation, cancellation and the 300-second deadline. The problem
is presentation and input routing, not policy.

## Decision

1. Replace the full-screen clarify panel with a bottom clarify bar stacked
   above the input panel. The transcript stays visible and the composer keeps
   its own rows; the bar does not call `Clear` over the whole frame.
2. A single-question bar is 7 rows; a 1–5 question batch uses the same bar,
   shows one question at a time with a `第 x/y 题` title, and is 8 rows. A
   collapsed pending bar is one row.
3. Keys: `1`–`4` select an option directly; `↑/↓` move; `←/→` or `Tab` move
   between questions; `Enter` selects or confirms; `S`/space skips; `Esc`
   collapses (it no longer cancels); `C` or `Enter` expands a collapsed bar;
   `Ctrl+C` cancels. `PageUp`/`PageDown` scroll the transcript while the bar is
   open.
4. The recommended option remains first and is marked `推荐`; `Other` stays an
   inline editor inside the bar. The countdown is shown only in the last 60
   seconds.
5. `ClarifyPanel` and `TuiClarifyState` keep their existing selection,
   validation and expiry rules. `TuiClarifyState` gains a bounded
   `select_option(index, now)` helper for number-key selection; `TuiState`
   owns the collapsed flag.
6. Both `/clarify <questions.json>` and the model-initiated `clarify` tool use
   the bottom bar. The 300-second deadline still applies; a timed-out round
   keeps the default answer valid and finishes with `timed_out`.

## Consequences

- A one-question clarification no longer hides the conversation; the user can
  read the previous answer while choosing.
- The bottom stack is transcript → clarify bar → input panel. The composer
  stays visible; the bar adds 7–8 rows above it instead of replacing it.
  Narrow terminals clip the bar rather than switching to a modal.
- Esc is now a reversible “collapse”, not a destructive cancel. Cancellation
  is explicit via `Ctrl+C`, and the collapsed bar shows that hint.
- A collapsed bar still waits on the same 300-second deadline. Making the
  deadline pause while collapsed is future work; the current state machine
  does not expose that distinction.
- The TUI remains keyboard-only; mouse selection is not part of this slice.
- The old full-screen render and its test are removed. The bar has render
  tests for the transcript staying visible, collapse/expand, number-key
  selection and `Ctrl+C` cancellation.

## Evidence

```bash
cargo test --lib clarify
cargo test --lib number_selection_uses_the_option_index
cargo test --lib clarify_escape_collapses_and_c_expands_without_losing_transcript
cargo test --lib clarify_number_keys_select_an_option
cargo test --lib clarify_ctrl_c_still_cancels
```

The tests prove the prompt contract and input routing; they do not prove
real-terminal aesthetics on every width. A PTY visual check remains a manual
follow-up.
