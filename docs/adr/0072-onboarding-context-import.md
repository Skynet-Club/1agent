# ADR 0072: Onboarding Context Import

- Status: accepted
- Date: 2026-11-12

## Context

Onboarding can write a voice, autonomy levels, boundaries, a first goal and a
commitment, but the user cannot bring existing context into local memory. SA98-54
calls for context import after the profile interview.

## Decision

1. Add `--import-file PATH` and `--import-kind
   fact|preference|project|feedback` to `1agent onboard`.
2. File format is plain UTF-8 text:
   - each non-empty line becomes one semantic memory record;
   - lines starting with `#` after trimming are comments;
   - blank lines are skipped.
3. Limits are enforced before writing:
   - the file must be a regular file, not a symlink;
   - at most 256 KiB;
   - at most 200 usable lines;
   - at most 2000 characters per line;
   - an empty/comment-only file is an error.
4. Imported records use the `onboard` source and the same memory scope as the
   rest of onboarding. The JSON report adds `imported`.
5. This is a local text import. It does not redact secrets, parse calendars or
   email, deduplicate against existing memory, or make memory/goal writes
   atomic.

## Consequences

- A user can seed memory from a notes file without a new storage format.
- The user must avoid importing secrets; no secret scanner runs on import.
- Re-running the same import writes duplicate memories; deduplication is
  separate.
- Preference lines use the ordinary `voice_mode=` / `autonomy.<domain>=`
  syntax, so they can affect the derived user model exactly like interactive
  onboarding.
- This is not a document parser; PDFs, HTML and Office files must be converted
  first.

## Evidence

```bash
cargo test --test onboard_cli
```

The tests cover preference import affecting the user model, comment/blank
skipping, fact import counts, missing files, comment-only files and an invalid
kind.
