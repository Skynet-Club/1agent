# Skill Capture — Evidence (2026-11-12)

Status: **passed on this machine**
Scope: `skill_capture::capture_journal`, `1agent skill capture`.

## Commands

```bash
cargo test skill_capture --lib
cargo test --test skill_capture_cli
cargo test --test skill_scan_cli
cargo clippy --all-targets -- -D warnings
```

## Expected results

```text
running 9 tests
test skill_capture::tests::captures_a_completed_run_with_redaction_and_generalization ... ok
test skill_capture::tests::redacts_escaped_quotes_inside_secret_values ... ok
test skill_capture::tests::refuses_a_run_without_a_completed_stop ... ok
test skill_capture::tests::refuses_a_run_without_successful_tools ... ok
test skill_capture::tests::refuses_a_run_with_too_many_steps ... ok
test skill_capture::tests::rejects_an_oversized_journal_before_reading_it ... ok
test skill_capture::tests::rejects_frontmatter_injection_in_metadata ... ok
test skill_capture::tests::sanitizes_assignments_and_url_credentials ... ok
test skill_capture::tests::sanitizes_secret_named_keys_and_avoids_quadratic_collisions ... ok

running 3 tests
test capture_writes_a_scannable_draft_without_the_run_body ... ok
test capture_refuses_to_overwrite_unless_forced ... ok
test capture_rejects_an_incomplete_run ... ok
```

## What this proves

- Only the last run segment is considered, and it must end with
  `run_stopped { reason: completed }`. A cancelled run is rejected.
- Only tool calls with a matching `tool_observed { status: ok }` become
  procedure steps; failed calls and observation bodies are not copied.
- The generated draft contains `$HOME`, `$TMPDIR`, `<id>`, `<email>` and
  `<redacted>` instead of the fixture's home path, temp path, UUID, email,
  secret token and secret-named JSON value.
- The generated draft passes `skill scan` (`scan_markdown`).
- An existing output file is not overwritten unless `--force` is passed.
- A journal larger than 64 MiB is rejected from metadata and again by the
  bounded reader; non-regular files are rejected.
- More than 100 successful steps fail closed instead of producing a draft.
- Balanced object/array values under a secret-named key are redacted
  wholesale, including escaped quotes inside the value; hyphenated header
  names (`X-Api-Key`) match, and JSON object keys are sanitized/generalized
  (email, home path, backtick) even when the key is secret-named. Multiple
  keys that generalize to the same value get `<email>`, `<email>#2`,
  `<email>#3` without quadratic probing.
- URL userinfo uses the last `@`, so a password fragment after an earlier `@`
  is removed; an email followed by a period becomes `<email>.`.
- Multi-line metadata and duplicate frontmatter keys are rejected, and
  `SkillManifest::validate` rejects unsafe scopes and platform/tool entries.

## What this does not prove

- That the heuristic redaction catches every secret format; it is not a DLP
  system.
- That the captured procedure is correct, safe or useful; held-out
  evaluation, human review and activation are still required.
- That a need-evidence or failed run can be captured; these are deliberately
  rejected.
- Cross-platform behavior; tests ran on Linux only.

## Files

- `src/skill_capture.rs`
- `src/main.rs` (`skill capture`)
- `tests/skill_capture_cli.rs`
- `docs/adr/0084-skill-capture.md`
- `docs/architecture.md`, `docs/features.md`, `docs/features.en.md`,
  `docs/claims.md`, `CHANGELOG.md`
