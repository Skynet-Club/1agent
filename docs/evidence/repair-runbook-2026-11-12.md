# Repair Runbook — Evidence (2026-11-12)

Status: **passed on this machine**  
Scope: `repair::build_runbook` and `1agent repair-plan`.

## Commands

```bash
cargo test repair --lib
cargo test --test repair_plan_cli
cargo clippy --all-targets -- -D warnings
```

## Expected results

```text
repair unit tests include:
test repair::tests::runbook_counts_actions_and_fails_closed_on_missing_compensation ... ok
... existing repair tests ...

running 2 tests (tests/repair_plan_cli.rs)
test repair_plan_rejects_a_corrupt_store ... ok
test repair_plan_reports_an_empty_store_as_clean ... ok

test result: ok. 2 passed; 0 failed
```

## What this proves

- An empty ledger produces `clean: true`, zero plans and zero errors.
- A ledger with planned, compensate, manual and broken-compensation entries
  reports 3 plans, 1 error, 1 compensate, 1 manual and `clean: false`.
- The CLI succeeds with `--require-clean` on an empty store and fails on a
  corrupt store.

## What this does not prove

- That a compensation is executed; this is planning only.
- Authority/Policy/Approval around execution.
- A non-clean CLI fixture; the non-clean path is covered at the unit level.
- Cross-platform behavior; tests ran on Linux only.

## Files

- `src/repair.rs`
- `src/main.rs` (`repair-plan`)
- `tests/repair_plan_cli.rs`
- `docs/adr/0038-trust-and-repair.md`
- `docs/architecture.md`, `docs/features.md`, `docs/features.en.md`,
  `docs/claims.md`, `CHANGELOG.md`
