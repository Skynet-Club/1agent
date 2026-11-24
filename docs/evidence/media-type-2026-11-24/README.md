# `.rs` media type in verify_artifact — 2026-11-24

The last full eval's only miss was `bug-fix-last-index` repeat-3: the model
wrote `counter.rs`, called `verify_artifact` with
`media_type: "text/rust"`, and the tool detected
`application/octet-stream`. The hash and content checks passed, but the model
kept the delivery `conditional` because the media-type check failed.

## Fix

`detect_media_type` now maps:

- `.rs` → `text/rust`;
- `.txt` → `text/plain` (the same octet-stream fallback affected checksum-file
  runs earlier).

Other extensions are unchanged; add them when a real case requires it.

## Test

`tools::artifact::tests::verifies_rust_source_as_text` writes a `.rs` file,
verifies it with `media_type: "text/rust"` and the right sha256, asserts
`decision=verified`, and asserts `.txt` detects as `text/plain`.

## Verification

- `scripts/verify.sh` green; core eval 8/8.
- Focused bug-fix eval N=3 on binary `5ef5fb7c…`: **3/3, pass^3 100%**, no
  issue samples (`report.json`).
- Previous binary `7d338a8e…` saved as
  `~/.local/bin/1agent-pre-media-type-2026-11-24`.

## Evidence

- `bug-fix-suite.json`, `report.json`, `stderr.log`.
- `product-eval/` — per-repeat journals; ignored by Git.
