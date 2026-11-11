# ADR 0065: Organization Audit Index

- Status: accepted
- Date: 2026-11-12

## Context

Team role policy decides allow/deny, but nothing records those decisions in a
reviewable organization-level index. SA99-62 calls for an organization audit
index. The existing per-run journals are local to a run and not a shared
organization view.

## Decision

1. Add `src/org_audit.rs` with `OrgAuditEntry { seq, at_ms, actor, action,
   scope, decision, prev_hash, hash }`.
2. Each entry hashes a canonical body (`seq`, `at_ms`, actor, action, scope,
   decision, previous hash) with SHA-256. The first entry uses 64 zeroes as the
   previous hash.
3. `append` validates non-empty actor/action, an `allow`/`deny` decision, and
   no control characters in the scope; it appends one JSON line and calls
   `sync_data` before returning.
4. `verify` checks contiguous sequence numbers, the previous-hash link and the
   recomputed hash for every entry, returning the first invalid sequence.
5. `1agent org-audit append|verify` exposes the store. A missing file is an
   empty, valid index; an invalid chain exits non-zero.
6. This is a local hash-chained index. It does not authenticate the actor,
   sign entries, anchor the head externally, or provide remote transparency.

## Consequences

- A team can review who was allowed or denied what, in order.
- Editing a field, swapping entries or breaking a link is detected on verify.
- An attacker who can rewrite the whole file can recompute the chain; without
  signing or external anchoring this is tamper-evident only for partial edits.
- The log grows forever; retention and compaction are not implemented.
- Multi-writer locking is not implemented. Append serializes at the file level
  only by relying on O_APPEND for single-line writes.

## Evidence

```bash
cargo test org_audit --lib
cargo test --test org_audit_cli
```

The tests cover chaining, reopen, edited fields, reordering, partial final
lines, invalid decisions/actors, and CLI append/verify across processes.
