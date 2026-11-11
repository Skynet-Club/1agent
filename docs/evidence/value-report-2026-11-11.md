# Value Report v0 — Evidence (2026-11-11)

Status: **passed on this machine**  
Scope: `value_report::build_value_report` and `1agent value-report`. This is a
fact report, not a time-saved or monetary-value claim.

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

- Runs are counted by stop reason: completed, needs authorization, needs
  evidence, failed, cancelled and other.
- Tokens, cost, tool calls/failures, alerts and open inbox items are reported.
- `cost_per_completed_micros` is cost divided by completed outcomes, and is
  zero when nothing completed.
- The window filters runs by modification time.
- `1agent value-report` reads a real mock-provider journal and prints JSON or
  human-readable output; an open inbox item increments `inbox_open`.
- `inbox_open` is a separate queue size and is not deduplicated against
  `alerts` or `pending_decisions`.

## What this does not prove

- That the report reflects a real user's value or time saved; no baseline was
  measured.
- That cost is complete; zero-cost calls may be unpriced providers.
- That the journal window equals the user's work window; it is a file-time
  filter.
- Cross-platform behavior; the test ran on Linux only.

## Files

- `src/value_report.rs`
- `src/main.rs` (`value-report`)
- `tests/value_report_cli.rs`
- `docs/adr/0039-value-report-v0.md`
- `docs/architecture.md`, `docs/features.md`, `docs/features.en.md`,
  `docs/claims.md`, `CHANGELOG.md`
