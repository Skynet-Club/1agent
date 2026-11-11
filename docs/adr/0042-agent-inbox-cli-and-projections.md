# ADR 0042: Agent Inbox CLI and Source Projections

- Status: accepted
- Date: 2026-11-11

## Context

ADR-0041 established the inbox store and state machine but left it without a
user surface or a way to turn existing runtime facts into items. The next slice
needs a minimal CLI and deterministic projections so the inbox can be used
before a desktop/chat surface exists.

## Decision

1. Add `inbox add|list|resolve|snooze`:
   - `add` creates a `notify`/`question`/`review` item;
   - `list` shows open or due-snoozed items, ordered review → question → notify;
   - `resolve` accepts `approved`, `rejected`, `answered` or `dismissed`;
   - `snooze` requires a future `until_ms`.
2. `InboxKind` and `InboxStatus` parse from lowercase names; an unknown value
   fails closed.
3. Add projections:
   - `project_attention`: overdue/missed commitments → `review`; due goals and
     commitments → `notify`. Item id is `attention:<candidate id>`.
   - `project_alerts`: alert kinds containing `fail`/`error` → `review`; other
     alerts → `notify`. Item id is `alert:<alert id>`.
4. Projections are pure functions. They do not write to the store; a later
   `inbox sync` command will merge them and skip existing ids.
5. The CLI is a local operator surface, not a remote API. It does not bypass
   Authority/Policy/Approval for any action represented by an item.

## Consequences

- The inbox can be exercised end to end with `add → list → snooze → resolve`.
- Attention and alert sources have stable, testable mapping rules.
- A user can see why an item exists (`why_now`) and where it came from
  (`source`).
- No dedupe-on-sync yet; the caller must skip existing ids. The next slice adds
  `inbox sync`.

## Evidence

```bash
cargo test inbox --lib
cargo test --test inbox_cli
```

The unit tests cover attention/alert projections and parsing; the CLI test
covers add/list/snooze/resolve and invalid kind/terminal transition failures.
