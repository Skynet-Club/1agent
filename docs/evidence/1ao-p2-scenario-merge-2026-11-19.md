# 1AO P2 — Scenario merge v0 (2026-11-19)

Scope: local scenario/proposal staging with a materialized full-file payload,
staged checks, approval-bound merge, backups and rollback. Unified-diff or
worktree payloads and a visual review UI are not implemented.

## Behavior

- `code.propose_change` accepts optional `changes: [{path, content}]` (UTF-8,
  512 KiB total). Each change is limited to paths in `paths`; targets must be
  regular files or new files with an existing parent inside the workspace.
  Symlinks and traversal are rejected.
- The proposal records, per change, `before_exists`, `before_sha256`,
  `after_sha256`, bytes and content, plus a deterministic `payload_sha256`.
  Writing the proposal never mutates the target.
- `code.run_checks` reports `mode: "staged"`, rechecks the current target
  against `before_sha256`, verifies the staged content hash, and records
  `payload_sha256`. Drift or a mismatched payload fails the report.
- `code.merge_change` requires:
  - a proposal with a non-empty staged payload;
  - an approved human receipt bound to the proposal SHA256;
  - a passing staged check report bound to the proposal and payload SHA256;
  - target files still matching their base hashes.
- Merge writes backups under `.1a/merges/<id>/backup`, replaces each target
  atomically (private temp file, fsync, rename), verifies after hashes and
  writes `merge.json`. If a later write fails, earlier files are restored.
- `code.merge_change` is registered as a typed tool and is model-visible only
  when the capability contract selects it. Proposal-only tasks advertise
  propose/check/package; a separate merge hint selects merge.

## Security hardening (review pass)

- `.1a` and `.git` components are rejected as staged targets at propose,
  check and merge time; canonical target/parent paths resolving into either
  control area are rejected too, so a symlink alias cannot forge approval/check
  artifacts or repository hooks.
- `.1a` control directories are created/verified component-by-component as
  real directories (no symlinked ledger), and proposal/approval/check/merge
  reads reject symlinked files and oversized JSON.
- The merge receipt is rechecked against proposal, payload, approval and check
  hashes; replay verifies every target is at its recorded after hash.
- A crash after all writes but before the receipt is completed on retry; a
  partial crash fails closed with the backup path instead of pretending success.
- A failed rollback is reported explicitly; it never claims rollback succeeded.
- Post-rename directory-fsync failures are treated as committed, so a committed
  write cannot be reported as "not applied".
- Each target is rehashed immediately before its atomic replace; parent
  containment is checked again at write time.

## Verification

| Check | Result |
|---|---|
| `cargo test --offline --lib tools::action::tests` | 23 passed |
| `cargo test --offline --lib` | 1234 passed, 0 failed, 4 ignored |
| `cargo test --offline --test ontology_cli --test tools_e2e` | 30 passed |
| `./scripts/verify.sh` | **1689 passed, 0 failed, 13 ignored** |
| claims | 271 (247 verified / 16 partial / 8 no evidence) |

Negative/atomicity tests: missing approval, target drift after checks,
change path absent from `paths`, read-only merge, control-directory/symlink
targets, crash-after-write recovery, partial-crash rejection, receipt replay
divergence, and a two-file merge where the second write fails and the first is
rolled back.

Raw: `verify-p2-merge.log`.

## Limits

- Payloads are full-file replacements, not unified diffs; no worktree branch or
  visual diff review.
- Checks are deterministic hash/existence checks, not a project test runner.
- Merge is process-atomic, not crash-atomic across a power loss. A retry
  completes a fully-applied-but-unreceipted merge; a partial crash is detected
  and refused with the backup path, not automatically recovered.
- TOCTOU is narrowed by a parent-containment check and a per-file rehash just
  before each rename, but remains for an adversarial same-user process racing
  the rename itself; there is no workspace lock or dirfd-based renameat.
- A merge receipt is idempotent by proposal id; it is not signed.
- Backups are written before mutation but are not yet consumed by an automatic
  partial-merge repair tool.
