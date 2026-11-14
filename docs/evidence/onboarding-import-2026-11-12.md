# Onboarding Context Import — Evidence (2026-11-12)

Status: **passed on this machine**  
Scope: `1agent onboard --import-file/--import-kind`.

## Commands

```bash
cargo test --test onboard_cli
cargo clippy --all-targets -- -D warnings
```

## Expected results

```text
running 7 tests (tests/onboard_cli.rs)
test onboard_can_seed_a_goal_and_commitment_for_inbox_sync ... ok
test onboard_commitment_without_goal_fails_closed ... ok
test onboard_import_rejects_bad_files_and_kinds ... ok
test onboard_imports_plain_text_context ... ok
test onboard_interactive_reads_stdin_answers ... ok
test onboard_rejects_invalid_autonomy_and_voice ... ok
test onboard_writes_profile_and_user_model_reads_it ... ok

test result: ok. 7 passed; 0 failed
```

## What this proves

- A file with a comment and two preference lines reports `imported: 2` and
  makes `voice_mode=advisor` visible in the derived user model.
- A fact import skips a `#` comment and imports two lines.
- Missing files, comment-only files and an invalid kind exit non-zero.

## What this does not prove

- That imported text is free of secrets or sensitive data.
- Deduplication, update semantics or deletion on re-import.
- Calendar/email/PDF/HTML parsing.
- Atomicity across memory and goal writes.
- Cross-platform behavior; tests ran on Linux only.

## Files

- `src/main.rs` (`onboard --import-file`, `read_import_lines`)
- `tests/onboard_cli.rs`
- `docs/adr/0072-onboarding-context-import.md`
- `docs/architecture.md`, `docs/features.md`, `docs/features.en.md`,
  `docs/claims.md`, `CHANGELOG.md`
