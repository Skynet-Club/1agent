# Status v0.6.0

Date: 2026-11-01
Status: bounded read-only workspace search

## Added since v0.5.0

- `search_files` read-only tool.
- Search path must canonicalize under a configured read root.
- Symbolic links are not followed.
- Common generated directories are skipped.
- Traversal limits:
  - maximum depth 32;
  - maximum file size 1 MiB;
  - maximum visited files 2,000;
  - maximum results 100.
- Results include relative file path, line, column and bounded excerpt.
- Files containing hidden or bidirectional controls are excluded.
- UTF-8-invalid and oversized files are skipped.

## Verification

| Check | Result |
|---|---|
| Unit tests | 34 passed |
| Search match and line reporting | passed |
| Generated-directory exclusion | passed |
| Read-root escape rejection | passed |
| Existing run/config/experience/resume suites | passed |
| Clippy `-D warnings` | passed |

## Requirement comparison

| Requirement | Status |
|---|---|
| Read-root confinement | implemented |
| Symlink exclusion | implemented |
| Generated-directory exclusion | implemented |
| Bounded traversal | implemented |
| Result provenance | implemented, file/line/column/excerpt |
| Hidden control rejection | implemented |
| HTTP/Web fetch | not started |
| Real-model quality | not verified |

## Known risks

- Search is literal substring matching, not BM25 or semantic retrieval.
- Binary detection is currently UTF-8 decoding rather than content-type inspection.
- Case-insensitive matching uses Unicode lowercase expansion and reports the folded match
  position; grapheme-level highlighting is not guaranteed.
- Search skips files above the configured size instead of indexing them.

## Next requirement

Build an offline evaluation harness and report:

1. Run deterministic end-to-end scenarios from versioned JSON cases.
2. Measure task success, evidence integrity, action safety, journal audit and resume behavior.
3. Produce machine-readable and human-readable scorecards.
4. Fail the process on any hard-gate violation.
5. Keep real-provider evaluation as a separate opt-in suite.
