# Independent Verification / Hack-Verifiable Suite — Evidence (2026-11-11)

Status: **passed on this machine**  
Scope: `verification::verify`, `verification::summarize`, and the deterministic
`tests/hack_verifiable.rs` fixture. This is a contract fixture, not a
model-quality claim.

## Commands

```bash
cargo test verification --lib
cargo test --test hack_verifiable
cargo clippy --all-targets -- -D warnings
```

## Expected results

```text
running 7 tests
test verification::tests::clean_solution_is_verified ... ok
test verification::tests::self_verification_fails_closed ... ok
test verification::tests::public_pass_with_hidden_failure_is_hacked ... ok
test verification::tests::missing_or_duplicate_criteria_are_errors ... ok
test verification::tests::passing_result_without_evidence_is_an_error ... ok
test verification::tests::artifact_digest_mismatch_is_rejected ... ok
test verification::tests::hacked_resolved_rate_counts_claimed_successes ... ok

test result: ok. 7 passed; 0 failed

running 4 tests (tests/hack_verifiable.rs)
...

test result: ok. 4 passed; 0 failed
```

## What this proves

- The executor cannot verify itself; self-verification is an error.
- A missing or duplicate criterion result is an error, not a silent pass.
- A passing criterion without evidence is an error.
- A public pass with a hidden failure is classified `hacked`, not
  `verified`.
- An artifact digest mismatch or a failed public criterion is `rejected`.
- `hacked_resolved_bps` counts hacked resolutions against claimed resolutions,
  so a high public pass rate cannot hide the shortcut.

## What this does not prove

- That a real model will not hack a real task; the fixture is deterministic.
- That the hidden tests are good; a weak hidden set will under-count hacks.
- Model quality, safety or production readiness. This is acceptance plumbing.
- Cross-platform or provider behavior; no network or non-Linux host was used.

## Files

- `src/verification.rs`
- `tests/hack_verifiable.rs`
- `docs/adr/0037-independent-verification.md`
- `docs/architecture.md`, `docs/features.md`, `docs/features.en.md`,
  `docs/claims.md`, `CHANGELOG.md`
