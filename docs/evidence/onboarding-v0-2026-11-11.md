# Onboarding v0 — Evidence (2026-11-11)

Status: **passed on this machine**  
Scope: `1agent onboard` writing voice, per-domain autonomy and boundaries into
memory, then reading the same profile through `1agent user-model`.

## Commands

```bash
cargo test --test onboard_cli
cargo clippy --all-targets -- -D warnings
```

## Expected results

```text
running 2 tests (tests/onboard_cli.rs)
test onboard_writes_profile_and_user_model_reads_it ... ok
test onboard_rejects_invalid_autonomy_and_voice ... ok

test result: ok. 2 passed; 0 failed
```

## What this proves

- `onboard --voice advisor --autonomy workspace=act --autonomy email=observe
  --boundary "never touch production"` writes four records and reports
  `written=4`.
- The returned `UserModel` has voice `advisor`, autonomy `workspace=act` and
  `email=observe`, and the boundary.
- A separate `1agent user-model` invocation reads the same values back from the
  memory journal/payloads.
- An invalid autonomy level or voice fails closed.

## What this does not prove

- That a real user finds the onboarding useful; no user study was run.
- That the profile is complete; document/calendar/email import and an
  interactive interview are later slices.
- Boundary dedupe: re-running onboarding can append the same boundary again.
- Cross-platform behavior; the tests ran on Linux only.

## Files

- `src/main.rs` (`onboard`, `write_onboard`)
- `tests/onboard_cli.rs`
- `docs/adr/0045-onboarding-v0.md`
- `docs/architecture.md`, `docs/features.md`, `docs/features.en.md`,
  `docs/claims.md`, `CHANGELOG.md`
