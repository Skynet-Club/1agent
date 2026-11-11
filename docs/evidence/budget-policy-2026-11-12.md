# Budget Policy — Evidence (2026-11-12)

Status: **passed on this machine**  
Scope: `budget::evaluate`, `budget::cost_per_verified_outcome`,
`1agent budget-check`, and runtime enforcement through `RunBudget`.

## Commands

```bash
cargo test budget --lib
cargo test --test budget_check_cli
cargo test cost_budget_stops token_budget_stops --lib
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
- A run whose provider response exceeds `max_cost_micros` stops with
  `StopReason::BudgetExhausted` and a cost-budget answer.
- A run whose provider response exceeds `max_tokens` stops with
  `StopReason::BudgetExhausted` and a token-budget answer.

## What this does not prove

- That every provider reports cost/tokens accurately; enforcement uses
  provider-reported usage.
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
