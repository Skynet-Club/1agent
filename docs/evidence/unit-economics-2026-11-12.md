# Unit Economics Calculator — Evidence (2026-11-12)

Status: **passed on this machine**  
Scope: `unit_economics::evaluate` and `1agent unit-economics`.

## Commands

```bash
cargo test unit_economics --lib
cargo test --test unit_economics_cli
cargo clippy --all-targets -- -D warnings
```

## Expected results

```text
running 3 tests
test unit_economics::tests::negative_contribution_has_no_break_even ... ok
test unit_economics::tests::positive_margin_and_break_even_round_up ... ok
test unit_economics::tests::zero_revenue_and_zero_outcomes_are_handled ... ok

test result: ok. 3 passed; 0 failed

running 2 tests (tests/unit_economics_cli.rs)
test unit_economics_cli_handles_losses_and_zero_revenue ... ok
test unit_economics_cli_reports_margin_and_break_even ... ok

test result: ok. 2 passed; 0 failed
```

## What this proves

- Revenue 10000, variable 4000, fixed 9000, outcomes 10 gives contribution
  6000, net −3000, margin 6000 bps, per-outcome 1000/400 and break-even 15.
- A negative contribution reports a negative margin and no break-even.
- Zero revenue reports a null margin; zero outcomes reports null per-outcome
  fields.
- The CLI emits the same fields as JSON.

## What this does not prove

- That the supplied revenue/cost numbers are real or complete.
- A specific pricing plan, quota, usage pack or BYOK rate.
- Gross-margin dashboarding or trend analysis.
- Tax, refunds, churn, acquisition cost or currency effects.
- Cross-platform behavior; tests ran on Linux only.

## Files

- `src/unit_economics.rs`
- `src/main.rs` (`unit-economics`)
- `tests/unit_economics_cli.rs`
- `docs/adr/0077-unit-economics-calculator.md`
- `docs/architecture.md`, `docs/features.md`, `docs/features.en.md`,
  `docs/claims.md`, `CHANGELOG.md`
