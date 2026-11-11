# Budget Policy — Evidence (2026-11-12)

Status: **passed on this machine**  
Scope: `budget::evaluate`, `budget::cost_per_verified_outcome` and
`1agent budget-check`. This is a policy/CI helper; runtime enforcement is not
wired yet.

## Commands

```bash
cargo test budget --lib
cargo test --test budget_check_cli
cargo clippy --all-targets -- -D warnings
```

## Expected results

```text
running 4 tests
test budget::tests::unset_limits_are_unbounded ... ok
test budget::tests::under_limit_is_ok_and_eighty_percent_warns ... ok
test budget::tests::exceeding_any_limit_fails_the_check ... ok
test budget::tests::cost_per_verified_outcome_handles_zero ... ok

test result: ok. 4 passed; 0 failed

running 2 tests (tests/budget_check_cli.rs)
test budget_check_reports_ok_near_limit_and_unit_cost ... ok
test budget_check_exceeds_and_exits_nonzero ... ok

test result: ok. 2 passed; 0 failed
```

## What this proves

- Unset limits are unbounded.
- Under-limit is `ok`; 80% is `near_limit` but still allowed.
- Exceeding any limit is `exceeded`, not allowed, with an explicit failure.
- `cost_per_verified_outcome` is `None` for zero verified outcomes.
- `budget-check` exits non-zero on an exceeded budget.

## What this does not prove

- That a live run stops when the budget is exceeded; runtime wiring is pending.
- That the configured limits are appropriate; that is a product decision.
- That cost reporting is complete; zero-cost calls may be unpriced.
- Cross-platform behavior; the tests ran on Linux only.

## Files

- `src/budget.rs`
- `src/main.rs` (`budget-check`)
- `tests/budget_check_cli.rs`
- `docs/adr/0055-budget-policy.md`
- `docs/architecture.md`, `docs/features.md`, `docs/features.en.md`,
  `docs/claims.md`, `CHANGELOG.md`
