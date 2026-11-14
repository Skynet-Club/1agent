# Text File Attachments — Evidence (2026-11-12)

Status: **passed on this machine**
Scope: `attachment::load_attachments`, `format_attachment_block`,
`1agent run --attach`.

## Commands

```bash
cargo test attachment --lib
cargo test --test attachment_cli
cargo clippy --all-targets -- -D warnings
```

## Expected results

```text
running 3 tests
test attachment::tests::loads_utf8_text_and_records_the_hash ... ok
test attachment::tests::rejects_unsafe_sizes_types_and_names ... ok
test attachment::tests::rejects_a_non_file_path ... ok

running 2 tests (tests/attachment_cli.rs)
test attach_reaches_the_journaled_model_request_as_untrusted_data ... ok
test attach_rejects_binary_and_missing_files_before_the_run ... ok
```

## What this proves

- A UTF-8 file is loaded with its basename, byte length and SHA-256; the
  rendered block contains the untrusted-data label and per-file markers.
- A file larger than 1 MiB, a file containing NUL bytes, invalid UTF-8, a
  directory and more than five paths are rejected.
- `run --attach` journals the attachment block in `ModelRequested`, including
  the file name, text and hash.
- A binary or missing attachment fails before the run starts.

## What this does not prove

- That the model resists prompt injection inside the attached text; the label
  is a mitigation, not a sandbox.
- That attachments are redacted; they are not, so credentials in a file reach
  the provider and the journal.
- Image, screen, OCR or voice input; this slice is text files only.
- Cross-platform behavior; tests ran on Linux only.

## Files

- `src/attachment.rs`
- `src/runtime.rs` (`with_attachments`, initial system message)
- `src/main.rs` (`run --attach`)
- `tests/attachment_cli.rs`
- `docs/adr/0087-text-file-attachments.md`
- `docs/architecture.md`, `docs/features.md`, `docs/features.en.md`,
  `docs/claims.md`, `CHANGELOG.md`
