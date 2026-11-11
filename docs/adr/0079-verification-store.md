# ADR 0079: Verification Store

- Status: accepted
- Date: 2026-11-12

## Context

`verification::verify` produces a report, but the report was not persisted, so
the value report relied on a caller-supplied verified-outcome count. That
invites an ungrounded metric and makes historical verification invisible.

## Decision

1. Add `VerificationRecord { verification_id, run_id, at_ms, report }` and an
   append-only `VerificationStore` in `src/verification_store.rs`.
2. `append` validates non-empty ids, distinct executor/verifier, and a unique
   verification id; it writes one JSON line and syncs before returning.
3. `summary` reuses `verification::summarize` for verified/hacked/rejected/
   resolved and `hacked_resolved_bps`. `verified_count` counts only clean
   `Verified` outcomes in an inclusive time window.
4. Add `1agent verification record --store ... --verification-id ...
   --run-id ... --input INPUT.json` and `verification summary --store ...`.
5. Add `value-report --verification-store PATH`; it conflicts with
   `--verified-outcomes` rather than silently choosing one.
6. A final line without a newline is treated as an interrupted append and
   ignored.

## Consequences

- Cost per verified outcome can be derived from recorded verification reports.
- A report is stored only when a verifier actually produced it through
  `verification::verify`; the CLI does not accept a hand-written outcome.
- The store is local and append-only; there is no cross-machine aggregation,
  signature, retention or deduplication across run ids.
- `verification record` trusts the input file's executor/verifier identities;
  it does not authenticate them.

## Evidence

```bash
cargo test verification_store --lib
cargo test --test verification_store_cli
```

The tests cover verified/hacked/rejected summaries, bps, window counts,
duplicate/self-verification rejection, reopen, partial tails and the value
report integration.
