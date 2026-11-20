# MAST-inspired fault injection (2026-11-20)

Status: **7 deterministic negative tests pass**

## Why

MAST (“Why Do Multi-Agent LLM Systems Fail?”, NeurIPS 2025) finds that
verification/termination is a thin, leaky layer: premature termination,
no/incomplete verification and incorrect verification are common. The suite
maps those failure modes onto the Master/Work Item primitives.

## Mapping

| MAST failure mode | Test |
|---|---|
| FC3 premature termination / missing artifact | `missing_artifact_is_rejected` |
| FC3 no/incomplete verification | `missing_criterion_result_is_rejected` |
| FC3 incorrect verification (fake SHA) | `fake_artifact_sha_is_rejected` |
| FC3 self-verification | `self_verification_is_rejected_by_acceptance` |
| FC1 specification / decomposition coverage | `task_contract_coverage_failure_is_rejected` |
| FC2 inter-agent misalignment (duplicate/stale receipt) | `duplicate_and_stale_receipts_fail_closed` |
| FC3 no progress / loop | `repeated_blocked_observations_stall_and_replan` |

## Run

```bash
cargo test --test mast_fault_injection
```

Result: **7 passed / 0 failed**.

Additional gates:

- `cargo test --lib`: 1323 passed / 0 failed / 4 ignored.
- `cargo clippy --tests`: clean.

## What this proves

- Fake artifact SHA, missing artifact, missing criterion result, self-verify,
  stale/duplicate receipt, uncovered Work Item and repeated blocked actions all
  fail closed or move to `Replanning`.
- The fault cases are deterministic and do not need Workshop, network or a
  model.

## What this does not prove

- It is not a full MAST trace annotation of live multi-agent runs; it targets
  the Master/Work Item verification primitives.
- Hidden criteria and automatic verifier selection are not exercised.
- `Replanning` is recorded; automatic replan execution is still caller-owned.
- No Clarification-gate or lease-heartbeat fault cases yet.

## Next

1. Clarification gate (ambiguity before decomposition).
2. MA-005 lease heartbeat/reaper for silently running workers.
