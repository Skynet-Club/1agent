# ADR 0041: Agent Inbox Store — Durable Notify/Question/Review Queue

- Status: accepted
- Date: 2026-11-11

## Context

The partner needs one place where the user can see what it wants them to know,
answer or approve. A notification feed is not enough: an inbox item has state,
a source, a reason it is here now and a resolution. The user must be able to
snooze it, and the history must remain inspectable.

The runtime already has alerts, attention candidates and approvals, but no
single durable queue that can hold all three with an explicit status machine.

## Decision

1. Add `src/inbox.rs`.
2. An `InboxItem` has:
   - `kind`: `notify`, `question` or `review`;
   - `title`, `why_now`, `source`;
   - `created_ms`, `updated_ms`, optional `due_ms`;
   - `status`: `open`, `snoozed`, `approved`, `rejected`, `answered`,
     `dismissed`;
   - optional `snoozed_until_ms` and `resolution`.
3. `InboxStore` is append-only JSONL. Every mutation writes a full snapshot;
   replay keeps the latest snapshot per `item_id`. This is a first step, not a
   hash-chained ledger; a later slice can add a chain without changing the
   item contract.
4. Invariants:
   - `item_id`, `title`, `why_now` and `source` are non-empty;
   - duplicate `item_id` is an error;
   - a terminal status cannot transition;
   - a status change cannot precede the last update;
   - `snooze` requires `until_ms > at_ms`.
5. `open_sorted(now)` returns open items plus snoozed items whose deadline has
   passed, ordered `review` → `question` → `notify`, then due time, then
   creation time.
6. This slice is the domain and store only. CLI/TUI actions and projections
   from alerts/attention/approvals are the next slices.

## Consequences

- The inbox has a stable, testable state machine before any UI is attached.
- A snoozed item disappears from the open view but remains in the store.
- The append-only snapshot format is easy to audit and migrate.
- Without a hash chain, tampering is detectable only by file-level controls;
  this is explicitly a v0 limitation.

## Evidence

```bash
cargo test inbox --lib
```

The tests cover add/reload, duplicate and invalid items, terminal transitions,
snooze expiry and kind/due sorting.
