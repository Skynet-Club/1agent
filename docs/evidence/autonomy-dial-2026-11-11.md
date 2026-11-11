# Autonomy Dial Policy — Evidence (2026-11-11)

Status: **passed on this machine**  
Scope: `autonomy::decide`, `autonomy::decide_for` and `1agent autonomy`.
This is a policy module; it does not execute actions.

## Commands

```bash
cargo test autonomy --lib
cargo test --test autonomy_cli
cargo clippy --all-targets -- -D warnings
```

## Expected results

```text
running 5 tests
test autonomy::tests::observe_and_suggest_never_act ... ok
test autonomy::tests::act_allows_only_low_risk_reversible ... ok
test autonomy::tests::auto_allows_low_and_medium_reversible ... ok
test autonomy::tests::explicit_approval_covers_act_and_auto ... ok
test autonomy::tests::user_model_domain_level_is_used ... ok

test result: ok. 5 passed; 0 failed

running 2 tests (tests/autonomy_cli.rs)
test autonomy_cli_applies_risk_and_reversibility ... ok
test autonomy_cli_rejects_invalid_level_and_risk ... ok

test result: ok. 2 passed; 0 failed
```

## What this proves

- `observe` and `suggest` never act, even with explicit approval.
- `act` executes only low-risk reversible actions; medium/high risk or
  irreversible actions require approval.
- `auto` executes low/medium-risk reversible actions; high risk or
  irreversible actions require approval.
- Explicit approval covers `act` and `auto`, including high-risk/irreversible
  actions.
- `decide_for` reads the per-domain preference; an unset domain defaults to
  `suggest`.
- The CLI outputs the decision and rejects invalid level/risk.

## What this does not prove

- That the runtime actually enforces the decision on every tool; this slice is
  the policy, and Authority/Policy/ExecutionGate enforcement is separate.
- That the user's preference was set correctly; the UserModel tests cover
  parsing, not UI.
- Cross-platform behavior; the tests ran on Linux only.

## Files

- `src/autonomy.rs`
- `src/main.rs` (`autonomy`)
- `tests/autonomy_cli.rs`
- `docs/adr/0044-autonomy-dial.md`
- `docs/architecture.md`, `docs/features.md`, `docs/features.en.md`,
  `docs/claims.md`, `CHANGELOG.md`
