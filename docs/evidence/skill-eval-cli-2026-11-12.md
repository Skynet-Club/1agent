# Skill Evaluation CLI Gate — Evidence (2026-11-12)

Status: **passed on this machine**
Scope: `1agent skill register`, `skill eval`, `skill activate-if-passed`.

## Commands

```bash
cargo test --test skill_register_eval_cli
cargo test skill --lib
cargo clippy --all-targets -- -D warnings
```

## Expected results

```text
running 3 tests
test register_eval_and_activate_if_passed ... ok
test failing_eval_reports_and_does_not_activate ... ok
test invalid_suite_fails_closed ... ok

test result: ok. 3 passed; 0 failed
```

## What this proves

- A valid `SKILL.md` is scanned and registered as `release@1.0.0`; registering
  the same version again fails closed.
- `skill eval` reports `passed: true`, `score_bps: 10000` and the per-case
  result for a suite requiring `verify` and forbidding `rm -rf`.
- `skill activate-if-passed` activates only after that passing report; the
  skill journal contains an activation event for `1.0.0`.
- A skill body containing `rm -rf` evaluates as failed, reports the forbidden
  hit, and `activate-if-passed` exits non-zero without appending an activation
  event.
- An invalid suite (`suite_id` with a space, `min_score_bps` above 10000, no
  cases) fails closed.

## What this does not prove

- That the skill is safe to execute; execution still goes through tool
  permissions and Authority/Policy.
- That the suite is a good quality signal; marker matching can be gamed and
  the operator supplies the suite.
- Runtime canary promotion or rollback; those remain separate.
- Cross-platform behavior; tests ran on Linux only.

## Files

- `src/main.rs` (`skill register|eval|activate-if-passed`)
- `tests/skill_register_eval_cli.rs`
- `docs/adr/0085-skill-eval-cli-gate.md`
- `docs/architecture.md`, `docs/features.md`, `docs/features.en.md`,
  `docs/claims.md`, `CHANGELOG.md`
