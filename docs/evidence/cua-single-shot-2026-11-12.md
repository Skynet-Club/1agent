# CUA Single-Shot Plan — Evidence (2026-11-12)

Status: **passed on this machine**
Scope: `cua::CuaPlan`, `authorize`, `replan`.

## Commands

```bash
cargo test cua --lib
cargo test --test cua_check_cli
cargo clippy --all-targets -- -D warnings
```

## Expected results

```text
running 8 tests
test cua::tests::single_shot_plan_must_be_frozen_and_rejects_replanning ... ok
test cua::tests::invalid_plans_are_rejected ... ok
test cua::tests::red_team_cases_fail_closed ... ok
...
```

## What this proves

- An unfrozen plan denies every action, even a trusted low-risk click.
- After `freeze`, a planned low-risk action is allowed; an unplanned branch is
  denied.
- `replan` after freeze is denied and the frozen branch list is unchanged.
- Empty/duplicate/invalid branches and an empty/invalid plan id fail closed.
- The existing provenance, planner-quarantine and branch-steering tests still
  pass.

## What this does not prove

- That a real browser executor calls `authorize` before every action; there is
  no browser execution in this slice.
- That screenshots, clicks or typing are safe; those are not implemented.
- That a frozen plan is correct or complete; approval quality is a human and
  evaluation concern.
- Cross-platform behavior; tests ran on Linux only.

## Files

- `src/cua.rs`
- `docs/adr/0089-cua-single-shot-plan.md`
- `docs/architecture.md`, `docs/features.md`, `docs/features.en.md`,
  `docs/claims.md`, `CHANGELOG.md`
