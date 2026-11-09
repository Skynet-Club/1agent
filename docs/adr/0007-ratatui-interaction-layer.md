# ADR 0007: Ratatui Interaction Layer

Date: 2026-11-02
Status: accepted

## Context

The V0/V1 terminal adapter used a hand-written screen buffer. It could not express the input
selection, clipboard, command-picker and large transcript interactions expected from a daily-use
agent terminal. Continuing to extend that renderer would couple layout, wrapping, selection and
terminal protocols into one fragile state model.

The OpenAI Codex CLI is Apache-2.0 and its TUI demonstrates a tested interaction architecture built
on Ratatui, Crossterm, grapheme-aware editing, mouse capture and structured transcript rendering.
Its full application frontend is not reusable independently because it depends on Codex's app-server
and product-specific state.

## Decision

Adopt the Codex interaction model at the presentation boundary, without adopting the Codex runtime:

- use Ratatui for terminal buffers, layout and differential rendering;
- use Crossterm for keyboard, mouse, resize and bracketed-paste events;
- implement grapheme-aware input state and internal selection in the 1A presentation layer;
- capture mouse gestures and provide explicit text selection and clipboard actions;
- keep the user instruction associated with the first visible answer in a sticky presentation bar;
- yield that bar when the source prompt itself is visible and never include it in transcript
  selection or copying;
- render a structured, scrollable transcript;
- filter and render slash commands in a composer-adjacent drawer.

`AgentRuntime`, providers, tools, permissions and the journal remain authoritative and independent.
The implementation reuses interaction concepts and selected algorithms from OpenAI Codex at commit
`a09812981de703f18f38015cd5d500ab373f616c`; attribution and license terms are recorded in
`THIRD_PARTY_NOTICES.md` and `LICENSES/Apache-2.0.txt`.

## Consequences

- The interactive terminal gains stable layout, selection, clipboard and scrolling behavior.
- The line-oriented path remains available for pipes, CI and unimplemented platform adapters.
- UI state no longer needs to know provider SDK details.
- Markdown-rich rendering, search and token-level streaming remain follow-up work.
- Windows and macOS raw interactive behavior still require target-host verification.
