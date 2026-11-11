# Verification Store — Evidence (2026-11-12)

Status: **passed on this machine**  
Scope: `verification_store::VerificationStore` and
`1agent verification record|summary`.

## Commands

```bash
cargo test verification_store --lib
cargo test --test verification_store_cli
cargo clippy --all-targets -- -D warnings
```

## Expected results

```text
running 3 tests
test verification_store::tests::appends_and_summarizes_verified_and_hacked ... ok
test verification_store::tests::duplicate_ids_and_self_verification_are_rejected ... ok
test verification_store::tests::records_survive_reopen_and_a_partial_tail_is_ignored ... ok

test result: ok. 3 passed; 0 failed

running 1 test (tests/verification_store_cli.rs)
test verification_store_records_summarizes_and_feeds_value_report ... ok

test result: ok. 1 passed; 0 failed
```

## What this proves

- One Verified and one Hacked report summarize as verified=1, hacked=1,
  resolved=2, bps=5000.
- Clean Verified counts are windowed; the Hacked report is not counted.
- Duplicate ids and self-verification are rejected.
- The store survives reopen and ignores a partial tail.
- `value-report --verification-store` reports 1 verified outcome and the
  window summary (verified=1, hacked=1, bps=5000); combining it with
  `--verified-outcomes` exits non-zero.
- Recording with `--inbox-store` creates exactly one review item for the
  Hacked report and none for the Verified report.

## What this does not prove

- That executor/verifier identities are authenticated.
- That the verification input reflects a real run.
- Cross-machine aggregation, signatures or retention.
- Cross-platform behavior; tests ran on Linux only.

## Files

- `src/verification_store.rs`
- `src/main.rs` (`verification`, `value-report --verification-store`)
- `tests/verification_store_cli.rs`
- `docs/adr/0079-verification-store.md`
- `docs/architecture.md`, `docs/features.md`, `docs/features.en.md`,
  `docs/claims.md`, `CHANGELOG.md`
