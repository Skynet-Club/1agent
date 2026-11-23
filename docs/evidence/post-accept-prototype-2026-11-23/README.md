# Post-run acceptance prototype — 2026-11-23

Status: **read-only prototype; no runtime, ledger or answer behavior changed.**

`scripts/post_accept.py` reads a run journal, verifies every recorded
`write_file`/`edit_file` artifact against its recorded sha256, checks
tool-call/result pairing, and checks the answer's claims and sources. When the
answer has no verified claims, it derives evidence from the journal's recorded
writes and verified artifacts. It emits a manifest with `accepted`,
`conditional` or `rejected`.

## Run

```bash
python3 scripts/post_accept.py --self-test
python3 scripts/post_accept.py --journal <run.jsonl> --json
python3 scripts/post_accept.py --results docs/evidence/<run>/results.json --json
```

Self-test covers: accepted artifact, tampered artifact, missing observation,
zero-claim answer with journal-derived evidence, directory evidence source, and
an unpinned terminal write.

## Results

| Results file | 1agent journals | accepted | conditional | rejected |
|---|---|---|---|---|
| baseline `phase4-2026-11-23` | 48 | 40 | 8 | 0 |
| fix `phase4-acceptance-fix-2026-11-23` | 48 | **43** | 4 | 1 |

The 24 Pi records have no 1agent journal and are skipped.

Fix confusion matrix against the runtime's suite-expected status:

| Runtime status met? | Post-accept decision | Count |
|---|---|---|
| yes | accepted | 37 |
| yes | conditional | 1 |
| no | accepted | **6** |
| no | conditional | 3 |
| no | rejected | 1 |

Post-run deterministic acceptance:

- agrees with 37 runtime-accepted runs;
- recovers 6 runs the runtime marked non-compliant:
  - `bug-fix-last-index` (full, needs_evidence)
  - `long-task-plan` (full, conditional ×2)
  - `config-retries` (full, needs_evidence ×2)
  - `checksum-file` (core7, conditional)
- conservatively holds 1 runtime-accepted run (`broken-json`, unpinned
  terminal command);
- leaves 3 conditional + 1 rejected.

Remaining fix failures:

- `journal:terminal_write_unpinned`: 4 (terminal command may have written but
  no `write_file`/`edit_file` artifact was recorded);
- `answer:present`: 1 (provider error; retry).

The derived evidence removed all `answer:claims` failures: zero-claim answers
with a recorded write or verified artifact now pass the evidence check from
journal facts.

## Delivery snapshot smoke

The runtime now records a `delivery_snapshot` event at final answer time:
workspace root, file paths, sha256 and bytes (capped at 2048 files / 16 MB,
skipping internal directories). Post-run acceptance verifies those hashes and
treats terminal writes as pinned when the snapshot is complete.

Four previously `terminal_write_unpinned` cases were rerun with the new binary
(`40ab0e85…`):

| Arm | Case | Decision | Snapshot |
|---|---|---|---|
| full | broken-json | accepted | 1 file, 0 mismatches |
| core7 | broken-json | accepted | 1 file, 0 mismatches |
| full | directory-summary | accepted | 2 files, 0 mismatches |
| core7 | directory-summary | accepted | 2 files, 0 mismatches |

Evidence: `snapshot-smoke/results.json` and `snapshot-smoke/post-accept.json`.

## Interpretation

This validates the core architecture: the journal plus recorded artifact hashes
and derived evidence are enough to accept most correct work deterministically,
without another model acceptance call. The runtime's own status is noisier: it
misses 6 correct runs, while post-accept misses only unpinned terminal writes
and a provider error.

The slow path still has real work:

1. Pin terminal writes: snapshot changed files, or require `write_file`/
   `edit_file` for deliverable artifacts.
2. Treat provider errors as retryable.
3. Add `delivered` + asynchronous acceptance queue and port the prototype to
   `1agent accept --journal`.

## Limits

- The prototype checks artifact hashes, tool pairing and answer citations, not
  semantic hidden criteria.
- Terminal writes without a recorded artifact are `conditional`, not accepted.
- Derived claims describe recorded facts, not the semantic correctness of the
  conclusion.
- N=48 1agent runs from one model/endpoint; directional only.
