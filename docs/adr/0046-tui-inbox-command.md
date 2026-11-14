# ADR 0046: TUI `/inbox` View and Inline Actions

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
3. It reads the default `inbox.jsonl` (`home_or_legacy`).
4. Inline actions are supported:
   - `/inbox resolve <id> <approved|rejected|answered|dismissed> [note]`
   - `/inbox snooze <id> <until_ms>`
   The same `parse_action` / `apply_action` contract is used by the CLI.
5. The command is available in every stage and during a run. Listing does not
   mutate state; an explicit resolve/snooze mutates only the inbox store.

## Consequences

- A TUI user can see and act on the queue without leaving the interface.
- The action parser fails closed on unknown actions, non-terminal resolve
  statuses, missing ids and invalid `until_ms`.
- No new persistence or authority path is introduced; the inbox store is the
  only mutation target.

## Evidence

```bash
cargo test inbox --lib
cargo test tui_commands --lib
```

The inbox tests cover inline action parsing; the TUI tests assert `/inbox` is a
core, non-panel command with inline args. The full TUI interaction check remains
the release-level evidence.
