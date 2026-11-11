# ADR 0069: Handoff Inbox Projection

- Status: accepted
- Date: 2026-11-12

## Context

Team handoffs are stored, but the addressee may never look at the handoff
store. The inbox already projects commitments and alerts into actionable
items. Open handoffs should use the same surface.

## Decision

1. Add `inbox::project_handoffs(handoffs, recipient, now_ms)`:
   - only `open` handoffs whose `to` equals the recipient;
   - kind `review`, because accepting is a decision;
   - item id `handoff:<id>`;
   - source `handoffs`;
   - title `Handoff from <from>: <summary>`, truncated to 500 characters;
   - `why_now` names the scope or says there is none.
2. `inbox sync` accepts `--handoff-store PATH --recipient NAME`. `--recipient`
   without the store is an error.
3. Sync stays dry-run by default, dedupes by item id, and never overwrites an
   existing item's status. It does not accept the handoff.
4. `sync` requires at least one of `--goal-journal`, `--alert-log` or
   `--handoff-store`.
5. `inbox reconcile --handoff-store PATH [--apply]` closes projected items:
   accepted handoffs become approved, cancelled handoffs become dismissed,
   open handoffs are untouched. Dry-run is the default.

## Consequences

- The addressee sees open handoffs in `inbox list` and the TUI `/inbox`.
- The projection is read-only; accepting still requires `handoff accept`.
- A handoff created before the item was dismissed will not reappear because
  dedupe is by stable id. That is consistent with the alert projection.
- The projection trusts the `to` field; the handoff store itself does not
  authenticate identities.
- Notifications and presence remain separate.

## Evidence

```bash
cargo test inbox --lib
cargo test --test handoff_inbox_cli
```

The tests cover recipient filtering, terminal exclusion, 500-character
truncation, dry-run, apply, second-sync dedupe, and `--recipient` validation.
