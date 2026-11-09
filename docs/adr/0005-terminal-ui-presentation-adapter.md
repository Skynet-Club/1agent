# ADR 0005: Terminal UI as a Presentation Adapter

Date: 2026-11-02
Status: accepted; raw TTY rendering extended by ADR 0006

## Context

The first interactive interface must expose the existing runtime without creating a second
execution path or coupling domain behavior to terminal rendering. It must remain usable through
the packaged single binary, cross-platform source, pipes and ordinary terminals.

## Decision

`1agent tui` is a presentation adapter over `AgentRuntime`. It owns only view state: the visible
transcript, status, cognitive mode, risk, provider label, run budget and latest user instruction.
It calls the same router, permissions, tools, answer contract and journal as one-shot CLI runs.

The V0 renderer uses portable ANSI output and line-oriented stdin. It adds no terminal-framework
dependency and keeps terminal-size fallback deterministic. Terminal-bound text is sanitized before
rendering.

## Consequences

- Runtime behavior and audit evidence are identical across CLI and TUI entry points.
- The interface binary remains portable and its view model is directly testable.
- V0 does not provide raw-mode editing, mouse navigation, scrollable panes or token streaming.
- A later renderer can replace the presentation layer without changing the runtime contracts.
- Rich terminal behavior must be introduced through a new explicit decision and target-host
  verification, not by silently coupling it into domain state.
