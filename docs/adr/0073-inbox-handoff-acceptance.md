# ADR 0073: Inbox Handoff Acceptance

- Status: accepted
- Date: 2026-11-12

## Context

Open handoffs project into the inbox (ADR-0069), but resolving the inbox item
does not accept the handoff. The user has to remember a second command, and
"approved" in the inbox can mean something different from the handoff state.

## Decision

1. `inbox resolve` accepts optional `--handoff-store PATH` and `--actor NAME`.
2. When either is present:
   - the item id must be `handoff:<id>`;
   - both flags are required;
   - the status must be `approved`;
   - the inbox item must exist;
   - the handoff store accepts the id for that actor.
3. The handoff is accepted first; only then is the inbox item resolved with a
   note (`accepted by <actor>` or the user's note). JSON output returns both
   `item` and `handoff`.
4. Resolving a `handoff:<id>` item as `approved` without the flags fails closed.
   Other terminal statuses only close the inbox item and leave the handoff
   open; the handoff domain has no "rejected" state yet.
5. This is a local two-store operation. It is not atomic: if the inbox append
   fails after the handoff accept, the handoff remains accepted and the inbox
   item remains open.

## Consequences

- One explicit approval records both the handoff acceptance and the inbox
  closure.
- A wrong actor or a terminal handoff fails before the inbox changes.
- A crash between the two writes can leave them inconsistent; replay/repair is
  not implemented.
- Rejecting/declining a handoff remains unsupported.
- Notifications and presence remain separate.

## Evidence

```bash
cargo test --test handoff_inbox_cli
```

The tests cover missing flags, a wrong actor, a successful accept that closes
the item, and the resulting handoff status.
