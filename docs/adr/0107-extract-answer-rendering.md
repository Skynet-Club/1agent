# ADR 0107: Extract Answer Rendering

- Status: accepted
- Date: 2026-11-12
- Related: RFC 0001, SA97-25

## Context

`main.rs` still owned the human answer renderer: full structured output, the
layered default, and three status-label helpers. That mixed presentation with
command dispatch and made the renderer testable only through a CLI process.

## Decision

1. Move `render_answer`, `render_answer_default`,
   `answer_status_label`, `answer_impact_label` and `claim_status_label` to
   `src/answer_render.rs`.
2. `main.rs` imports them; behavior and output strings are unchanged.
3. Add direct unit tests for the low-risk concise view, the high-risk inline
   evidence/flip view and the label mappings.
4. Keep `print_route_trace` in `main.rs` for now; it reads a journal and is
   part of the CLI adapter.

## Consequences

- `main.rs` loses about 90 lines; rendering is testable without spawning a
  process.
- The CLI e2e default/`--details` test still passes, so the move preserved
  behavior.
- `main.rs` remains large; command groups, TUI state and route-trace printing
  are still unsplit.
