# ADR 0043: Inbox Sync — Merge Projections with Dedupe and Dry-Run

- Status: accepted
- Date: 2026-11-11

## Context

ADRs 0041-0042 gave the inbox a store, CLI and pure projections, but nothing
merged projections into the store. Without sync, the inbox is empty unless the
user adds items by hand.

The merge must be safe to run repeatedly: the same attention candidate or alert
must not create duplicate items, and a dry-run must show what would change
without writing.

## Decision

1. Add `InboxStore::sync(items, apply)`:
   - validate every projected item;
   - count `scanned`;
   - skip ids that already exist (`skipped`);
   - if `apply`, append and insert the item; otherwise only report it;
   - return `InboxSyncReport { scanned, added, skipped, dry_run, added_items }`.
2. Add `1agent inbox sync`:
   - `--goal-journal` projects attention candidates (overdue/missed → review,
     due → notify);
   - `--alert-log` projects alerts (fail/error → review, other → notify);
   - at least one source is required;
   - dry-run by default; `--apply` writes.
3. Sync never updates an existing item's status. A terminal item stays
   terminal; a user decision is not overwritten by a re-projection.
4. Sync does not resolve, approve or execute anything. It only adds items to
   the queue.

## Consequences

- The inbox can be populated from real runtime facts with one command.
- Repeated sync is idempotent: the second run adds zero items.
- Dry-run lets an operator inspect new items before they appear.
- A changed projection reason for an existing id is ignored; updating reasons
  is a later, explicit operation.

## Evidence

```bash
cargo test inbox --lib
cargo test --test inbox_sync_cli
```

The unit test covers dry-run and apply dedupe; the CLI test creates a real
alert log, syncs it, lists the item, and proves the second apply adds nothing.
