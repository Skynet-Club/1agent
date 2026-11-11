# ADR 0046: TUI `/inbox` Read-Only View

- Status: accepted
- Date: 2026-11-11

## Context

The inbox backend, CLI and projections are implemented, but a TUI user had to
leave the interface to run `1agent inbox list`. The partner's first presence
surface should show the queue where the user already works.

## Decision

1. Add a core TUI command `/inbox` in the session group.
2. It is a transcript command, not a command panel: it prints the current open
   or due-snoozed items with kind, status, title and `why_now`.
3. It reads the default `inbox.jsonl` (`home_or_legacy`) and is read-only.
   Resolve/snooze remain CLI actions in this slice.
4. The command is available in every stage and during a run; it does not
   mutate session or task state.

## Consequences

- A TUI user can see the queue without leaving the interface.
- The command does not yet approve, reject or snooze; those are the next UI
  slice.
- No new persistence or authority path is introduced.

## Evidence

```bash
cargo test tui_commands --lib
```

The tests assert `/inbox` is registered as a core, non-panel command; the full
TUI interaction check remains the release-level evidence.
