# ADR 0068: Team Handoff

- Status: accepted
- Date: 2026-11-12

## Context

Team role policy and the organization audit index exist, but a person cannot
hand grounded work to another person. SA99-62 calls for shared context and
handoff. A handoff without evidence would let one person assert unverified
work to another.

## Decision

1. Add `Handoff { id, from, to, scope, summary, open_items, evidence,
   created_ms, status, accepted_ms, accepted_by }` with `open`/`accepted`.
2. `create` requires:
   - non-empty id, from, to and summary;
   - `from != to`;
   - at least one non-empty evidence reference (for example a journal or
     receipt id);
   - an open, unaccepted initial state;
   - a unique id.
3. Only the addressee may `accept`; an unknown handoff is not found, and a
   terminal handoff cannot transition again.
4. Snapshots are appended and synced to JSONL; reopening takes the latest
   snapshot per id. `list` is ordered by creation time then id.
5. `1agent handoff create|accept|list` exposes the store.
6. This is a local note exchange. Live shared context, presence, an IdP, and
   per-team access control on the store are separate.

## Consequences

- Handoffs carry evidence references instead of unsupported claims.
- Accepting is attributable and one-way.
- The store is a local file; anyone who can write it can forge `from`. The
  org audit index can record decisions separately, but the two are not
  cryptographically bound yet.
- No notifications are sent; the caller or inbox layer must surface open
  handoffs.
- No encryption at rest; the handoff summary should not contain secrets.

## Evidence

```bash
cargo test handoff --lib
cargo test --test handoff_cli
```

The tests cover create/accept/reopen, addressee enforcement, terminal
transitions, duplicate ids, missing evidence, empty summaries, self-handoff,
sorted listing and unknown ids.
