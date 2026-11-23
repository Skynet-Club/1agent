# Post-run acceptance prototype — 2026-11-23

Status: **read-only prototype; no runtime, ledger or answer behavior changed.**

`scripts/post_accept.py` reads a run journal, verifies every recorded
`write_file`/`edit_file` artifact against its recorded sha256, checks
tool-call/result pairing, and checks the answer's claims and sources. It emits
a manifest with `accepted`, `conditional` or `rejected`.

## Run

```bash
python3 scripts/post_accept.py --self-test
python3 scripts/post_accept.py --journal <run.jsonl> --json
python3 scripts/post_accept.py --results docs/evidence/<run>/results.json --json
```

Self-test covers: accepted artifact, tampered artifact, missing observation,
zero-claim answer, directory evidence source, and an unpinned terminal write.

## Results

| Results file | 1agent journals | accepted | conditional | rejected |
|---|---|---|---|---|
| baseline `phase4-2026-11-23` | 48 | 33 | 15 | 0 |
| fix `phase4-acceptance-fix-2026-11-23` | 48 | **40** | 7 | 1 |

The 24 Pi records have no 1agent journal and are skipped.

Fix confusion matrix against the runtime's suite-expected status:

| Runtime status met? | Post-accept decision | Count |
|---|---|---|
| yes | accepted | 37 |
| yes | conditional | 1 |
| no | accepted | 3 |
| no | conditional | 6 |
| no | rejected | 1 |

Post-run deterministic acceptance:

- agrees with 37 runtime-accepted runs;
- recovers 3 runs the runtime marked non-compliant (`long-task-plan` 2,
  `checksum-file` 1);
- conservatively holds 1 runtime-accepted run (`broken-json`, unpinned
  terminal command);
- leaves 6 conditional + 1 rejected.

Remaining fix failures:

- `answer:claims`: 3 (zero claims/verified claims; slow evidence builder can
  derive claims from journal tool observations);
- `journal:terminal_write_unpinned`: 4 (terminal command may have written but
  no `write_file`/`edit_file` artifact was recorded);
- `answer:present`: 1 (provider error; retry).

## Interpretation

This validates the core architecture: the journal and recorded artifact hashes
are enough to accept most correct work deterministically, without another model
acceptance call. The runtime's own status is noisier: it misses 3 correct runs
and one accepted run lacks a pinned artifact proof.

The slow path still has real work:

1. Build evidence/claims from journal tool observations when the fast answer has
   none.
2. Pin terminal writes: snapshot changed files, or require `write_file`/
   `edit_file` for deliverable artifacts.
3. Treat provider errors as retryable.
4. Only then add `delivered` + asynchronous acceptance queue and port the
   prototype to `1agent accept --journal`.

## Limits

- The prototype checks artifact hashes and answer citations, not semantic
  hidden criteria.
- Terminal writes without a recorded artifact are `conditional`, not accepted.
- A directory counts as a valid evidence source; source existence is not a
  semantic check.
- N=48 1agent runs from one model/endpoint; directional only.
