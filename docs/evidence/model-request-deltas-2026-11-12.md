# Model Request Deltas — Evidence (2026-11-12)

Status: **local pass**
Scope: journal size for model requests on a deterministic 100-step echo run.

## Commands

```bash
cargo test --lib --offline delta_checkpoints_keep_the_journal_linear -- --ignored --nocapture
cargo test --lib --offline reframe_journal_tests::model_request_deltas_reconstruct_the_full_request
cargo test --lib --offline reframe_journal_tests::audit_flags_an_orphan_model_request_delta
cargo test --lib --offline reframe_journal_tests::audit_flags_a_model_request_delta_prefix_overflow
cargo test --offline
cargo clippy --all-targets --offline -- -D warnings
cargo fmt --check
```

## Output

```text
journal total=329465 checkpoint actual=114899 full-checkpoint estimate=2986785 ratio=0.038
model-request actual=74911 full-request estimate=3028246 ratio=0.025
test runtime::tests::delta_checkpoints_keep_the_journal_linear ... ok
```

- Full suite: **1242 passed / 0 failed / 13 ignored**
- Clippy: 0 warnings
- `cargo fmt --check`: clean

## What this proves

- Consecutive requests are journaled as deltas; the reconstructed sequence
  equals the original requests (unit test).
- Orphan deltas and prefix overflows are rejected by audit (unit tests).
- On this 100-step echo run, model-request events are 2.5% of the full-copy
  estimate.

## What this does not prove

- Provider wire savings, CPU or memory savings.
- Real-model transcripts or compaction-heavy runs; those produce larger deltas.
- Compatibility with binaries older than this change; they cannot read the new
  event variant.
