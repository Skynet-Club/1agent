# ADR 0024: Runtime Home and Export Boundary

- Status: accepted for the S-Gate productization slice
- Date: 2026-11-08

## Context

The CLI historically accepted per-command relative paths (`.journal`,
`.1a/task-graph.jsonl`, `.1a/payloads`). That is workable for tests but not for
a standalone product: a user cannot initialize, inspect, back up, upgrade or
remove one well-defined runtime home. Backups also need an explicit boundary
because the memory master key lives beside encrypted payloads.

## Decision

1. The default runtime home is `$ONEAGENT_HOME`, otherwise `$HOME/.1a`. A
   `home.json` marker binds the directory to schema `1agent.home.v1` and a
   version number.
2. `1agent init` creates the home and `journals/`, `payloads/`, `backups/` and
   `keys/` with mode 0700. It is idempotent. A non-empty directory without the
   marker is refused instead of adopted, so a mistyped `--root` cannot overwrite
   user files. Home layout version 2 keeps key material under `keys/`.
3. `1agent doctor` reports actionable checks (marker/version, directory
   permissions, layout, master-key permissions, writability) and verifies the
   hash chain of any task-graph, memory or skill journal present in the home.
   It exits non-zero on a failing check and does not repair state.
4. `1agent export` writes a directory bundle with an `export.json` manifest
   containing relative path, size and SHA-256 for every file. Key material
   (including `memory.key-keks.json`) and credential-looking files are excluded
   and listed under `excluded`. Files are written 0600 and the output must be
   empty and outside the home. A non-UTF-8 filename is rejected before any file
   is written, because a lossy manifest path cannot be restored.
5. `1agent restore` verifies the manifest, rejects absolute/`..` paths, extra
   files, symlinks, key material and hash mismatches, and writes only into an
   empty or nonexistent home. The home marker is written last, so a partial
   restore is not mistaken for an initialized home.
6. Export output and the home are compared after resolving the nearest existing
   ancestor, so a symlinked parent cannot smuggle the output back inside the
   home.
7. `1agent upgrade --check` reports missing layout without changing state.
   Version 1 to 2 moves `memory.key` and `memory.key-keks.json` into `keys/`
   with fsyncs and refuses to overwrite an existing destination.
   `1agent upgrade --rollback` moves them back, removes the empty `keys/`
   directory and writes version 1; unexpected files in `keys/` abort the
   rollback. `doctor` warns on a version-1 home (migration available) and fails
   on versions without a migration.
8. `1agent uninstall` never deletes data without `--purge --yes`. Purge requires
   a valid home marker and refuses `/`, top-level paths, `$HOME` and a home that
   contains the current working directory.
9. Exports deliberately exclude the master key, so an export alone cannot
   decrypt memory payloads. Key escrow remains a separate product decision and
   is not claimed by this ADR.

## Consequences

- The product has a single, testable home lifecycle without adopting the key.
- CLI path defaults resolve against an initialized home; explicit flags still
  override them. Uninitialized worktrees keep the historical relative paths, so
  starting `1agent init` is the opt-in boundary and old data is not silently
  reinterpreted.
- An export is a file-by-file copy with per-file hashes, not a transactional
  snapshot. Run it while the home is quiescent; a writer or key rotation during
  export can produce a manifest that verifies file-by-file but does not
  represent one consistent point in time.
- Version 2 is a real layout migration (key material moves under `keys/`), not
  a no-op version bump. The rollback is file-move based and requires no writer
  to be active; it does not merge concurrent changes.
- Already-built version-1 homes require `1agent upgrade` before this binary's
  default key paths, export/restore and key escrow operate on them.
