# Skill Canary CLI — Evidence (2026-11-12)

Status: **passed on this machine**
Scope: `1agent skill canary`, `SkillCanary` sample parsing.

## Commands

```bash
cargo test skill_canary --lib
cargo test --test skill_canary_cli
cargo clippy --all-targets -- -D warnings
```

## Expected results

```text
running 5 tests
test skill_canary::tests::jsonl_samples_parse_and_fail_with_line_numbers ... ok
test skill_canary::tests::promotes_after_enough_healthy_samples ... ok
test skill_canary::tests::rejects_invalid_policy_and_unknown_version ... ok
test skill_canary::tests::rolls_back_on_failures_and_restores_baseline ... ok
test skill_canary::tests::rolls_back_on_latency_and_disables_without_baseline ... ok

running 4 tests (tests/skill_canary_cli.rs)
test failures_roll_back_to_the_baseline ... ok
test healthy_samples_promote_the_candidate ... ok
test malformed_samples_fail_with_the_line_number ... ok
test too_few_samples_stay_pending_and_do_not_apply ... ok
```

## What this proves

- Four healthy samples at the configured threshold produce `state: promoted`
  and `decision.kind: promote`; the registry journal contains an activation
  for `1.1.0`.
- Two failures in four samples with a zero failure budget produce
  `state: rolled_back`; the journal contains no activation for `1.1.0`.
- One sample below `min_samples` produces `state: running` and
  `decision.kind: pending`; no activation is appended.
- A malformed sample reports its line number and exits non-zero.
- `parse_canary_samples` rejects unknown fields and bounds the sample count.

## What this does not prove

- That the samples represent live traffic; the caller supplies them.
- That the candidate skill is safe or high quality; this is a decision gate,
  not a quality measurement.
- That canary state survives between invocations; it does not.
- Cross-platform behavior; tests ran on Linux only.

## Files

- `src/skill_canary.rs`
- `src/main.rs` (`skill canary`)
- `tests/skill_canary_cli.rs`
- `docs/adr/0088-skill-canary-cli.md`
- `docs/architecture.md`, `docs/features.md`, `docs/features.en.md`,
  `docs/claims.md`, `CHANGELOG.md`
