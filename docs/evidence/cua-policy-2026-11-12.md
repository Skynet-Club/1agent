# CUA Provenance Policy — Evidence (2026-11-12)

Status: **passed on this machine**  
Scope: `cua::evaluate`, `cua::planner_sees_untrusted`, `cua::check_branch` and
`1agent cua-check`. This is a policy contract; browser execution is not
implemented.

## Commands

```bash
cargo test cua --lib
cargo test --test cua_check_cli
cargo clippy --all-targets -- -D warnings
```

## Expected results

```text
running 5 tests
test cua::tests::trusted_low_risk_is_allowed ... ok
test cua::tests::untrusted_argument_requires_approval_or_is_denied ... ok
test cua::tests::irreversible_medium_risk_requires_approval ... ok
test cua::tests::empty_action_is_denied ... ok
test cua::tests::planner_and_branch_guards_fail_closed ... ok

test result: ok. 5 passed; 0 failed

running 2 tests (tests/cua_check_cli.rs)
test cua_check_applies_provenance_and_risk ... ok
test cua_check_blocks_branch_steering ... ok

test result: ok. 2 passed; 0 failed
```

## What this proves

- Trusted low-risk actions are allowed.
- An untrusted argument on a low/medium risk action requires approval; on a
  high-risk action it is denied.
- An irreversible medium-risk action requires approval.
- An empty action name is denied.
- `planner_sees_untrusted(true)` is denied.
- An unplanned branch target is denied; a planned target is allowed.
- The CLI exposes the same decisions.

## What this does not prove

- That a real browser page cannot steer the agent; no CUA implementation exists.
- That screenshot/OCR content is tagged correctly at runtime; the policy
  assumes the caller supplies provenance.
- That the model follows the approval gate; Authority/Policy/ExecutionGate
  enforcement is separate.
- Cross-platform behavior; the tests ran on Linux only.

## Files

- `src/cua.rs`
- `src/main.rs` (`cua-check`)
- `tests/cua_check_cli.rs`
- `docs/adr/0053-cua-provenance-policy.md`
- `docs/architecture.md`, `docs/features.md`, `docs/features.en.md`,
  `docs/claims.md`, `CHANGELOG.md`
