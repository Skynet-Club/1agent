# ADR 0006: Raw Terminal Presentation

Date: 2026-11-02
Status: superseded by ADR 0007

## Context

The V0 terminal adapter used line-oriented stdin and full-screen clearing. That was portable and
simple, but it could not provide editable input, viewport scrolling or stable redraws for long
answers. A framework dependency was unavailable in the offline build environment.

## Decision

On Unix TTYs, `1agent tui` uses a small raw-terminal adapter implemented over `libc` termios and
portable ANSI cursor controls. The view model remains platform-neutral and testable:

- input editing and scroll state live in `TuiState`;
- terminal rows are wrapped with Unicode display width;
- a screen painter emits only changed rows;
- raw mode and alternate-screen ownership are restored on every exit path;
- piped stdin/stdout continue to use the V0 line-oriented path.

The domain runtime, provider router, tools, permissions and journal contracts are unchanged.

## Consequences

- Interactive terminals get editable input, scrolling and flicker-resistant redraws.
- CI and scripted hosts retain deterministic line-oriented behavior.
- macOS uses the same Unix adapter but still requires target-host verification.
- Windows currently falls back to the line-oriented path until a dedicated raw terminal backend
  is verified.
- Token streaming and mouse support remain outside this decision.
