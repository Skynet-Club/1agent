# Cost per Verified Outcome — Evidence (2026-11-12)

Status: **passed on this machine**  
Scope: `value_report::ValueReport` and `1agent value-report
--verified-outcomes`.

## Commands

```bash
cargo test value_report --lib
cargo test --test value_report_cli
cargo clippy --all-targets -- -D warnings
```

## Expected results

```text
running 2 tests
test value_report::tests::counts_outcomes_cost_and_pending_decisions ... ok
test value_report::tests::window_filters_and_zero_completed_has_zero_unit_cost ... ok

test result: ok. 2 passed; 0 failed

running 3 tests (tests/value_report_cli.rs)
test value_report_cli_reads_a_real_journal ... ok
test value_report_cli_human_output_is_readable ... ok
test value_report_includes_open_inbox_items ... ok

test result: ok. 3 passed; 0 failed
```

## What this proves

- With cost 900 micros and 2 verified outcomes, the unit cost is 450 micros.
- With no verified count, both new fields are `None`/`null`.
- The CLI accepts `--verified-outcomes` and emits both fields.
- Zero-cost runs still produce a numeric per-verified cost (0) rather than a
  null when the count is positive.

## What this does not prove

- That the supplied count is correct.
- That verification results are aggregated automatically from journals.
- That cost accounting covers every provider billing model.
- Cross-platform behavior; tests ran on Linux only.

## Files

- `src/value_report.rs`
- `src/main.rs` (`value-report --verified-outcomes`)
- `tests/value_report_cli.rs`
- `docs/adr/0071-cost-per-verified-outcome.md`
- `docs/architecture.md`, `docs/features.md`, `docs/features.en.md`,
  `docs/claims.md`, `CHANGELOG.md`
