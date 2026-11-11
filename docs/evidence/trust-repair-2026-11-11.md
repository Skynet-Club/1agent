# Trust & Repair — Evidence (2026-11-11)

Status: **passed on this machine**  
Scope: `repair::plan_repair`, `repair::receipt`, `repair::build_away_digest`
and `1agent away-digest`. This slice plans compensation; it does not execute it.

## Commands

```bash
cargo test repair --lib
cargo test --test away_digest_cli
cargo clippy --all-targets -- -D warnings
```

## Expected results

```text
running 6 tests
test repair::tests::planned_and_compensated_need_no_repair ... ok
test repair::tests::applied_reversible_effect_offers_compensation ... ok
test repair::tests::applied_reversible_without_compensation_fails_closed ... ok
test repair::tests::applied_irreversible_and_failed_require_manual_intervention ... ok
test repair::tests::away_digest_separates_states_and_surfaces_manual_work ... ok
test repair::tests::invalid_window_and_empty_fields_fail_closed ... ok

test result: ok. 6 passed; 0 failed

running 2 tests (tests/away_digest_cli.rs)
...

test result: ok. 2 passed; 0 failed
```

## What this proves

- `planned` and `compensated` effects need no repair.
- An applied reversible effect with a recorded compensation produces a
  `compensate` plan and a user receipt containing the undo instruction.
- An applied reversible effect **without** compensation is an error, not a
  silent irreversible action.
- An applied irreversible effect and a failed effect require manual review.
- The away digest separates applied, compensated, failed, pending and
  `needs_user`, and filters by the requested window.
- `1agent away-digest` reads a real task-graph store created by `task-graph
  demo` and prints the digest as JSON or text.

## What this does not prove

- That the compensation was executed correctly; execution is out of scope for
  this slice and still requires Authority/Policy/Approval.
- That a real user finds the receipt sufficient; no user study was run.
- That every side effect has a valid compensation string; this checks presence,
  not semantic correctness.
- Cross-platform behavior; the test ran on Linux only.

## Files

- `src/repair.rs`
- `src/main.rs` (`away-digest`)
- `tests/away_digest_cli.rs`
- `docs/adr/0038-trust-and-repair.md`
- `docs/architecture.md`, `docs/features.md`, `docs/features.en.md`,
  `docs/claims.md`, `CHANGELOG.md`
