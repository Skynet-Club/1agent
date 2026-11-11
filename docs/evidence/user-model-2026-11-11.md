# User Model 2.0 — Evidence (2026-11-11)

Status: **passed on this machine**  
Scope: `UserModel` parsing, ordering, consent, safe defaults, serialization and
the `1agent user-model` CLI. This is a contract test, not a user study.

## Commands

```bash
cargo test user_model --lib
cargo test --test user_model_cli
cargo clippy --all-targets -- -D warnings
```

## Results

```text
running 7 tests
test user_model::tests::builds_preferences_and_boundaries_with_sources ... ok
test user_model::tests::forgotten_autonomy_voice_and_consent_are_ignored ... ok
test user_model::tests::invalid_values_are_ignored ... ok
test user_model::tests::newer_autonomy_wins_and_older_is_ignored ... ok
test user_model::tests::autonomy_voice_and_consent_are_parsed ... ok
test user_model::tests::newer_preference_wins_and_forgotten_records_are_ignored ... ok
test user_model::tests::serialization_roundtrip_keeps_relationship_fields ... ok

test result: ok. 7 passed; 0 failed

running 2 tests
test user_model_cli_human_output_is_readable ... ok
test user_model_cli_exposes_relationship_preferences ... ok

test result: ok. 2 passed; 0 failed
```

`cargo clippy --all-targets -- -D warnings` completed with no warnings.

## What this proves

- `autonomy.<domain>=observe|suggest|act|auto` is parsed; unset domains default
  to `suggest`; newer memory records win; invalid values are ignored.
- `voice` / `voice_mode=operator|advisor|companion` is parsed; unset defaults to
  `operator`.
- `consent:<kind>=allow|deny[@<retention_ms>]` is parsed; default is allow;
  the latest rule per kind wins; forgotten/superseded records never grant more.
- The model round-trips through JSON with the new fields.
- `1agent user-model` prints the view as JSON and as human-readable text.

## What this does not prove

- That a real user finds the model correct or useful.
- That autonomy preferences change runtime behavior. They are preferences, not
  authorization grants; Authority/Policy/Approval still decide execution.
- Cross-platform behavior. This run is Linux only; Windows/macOS are `untested`
  until the P-Gate smoke exists.
- Real-provider behavior; the tests use the local memory service, no network.

## Files

- `src/user_model.rs`
- `src/main.rs` (`user-model` command)
- `tests/user_model_cli.rs`
- `docs/adr/0035-user-model-consent.md`
- `docs/architecture.md`
- `docs/features.md`, `docs/features.en.md`
- `docs/claims.md`
- `CHANGELOG.md`
