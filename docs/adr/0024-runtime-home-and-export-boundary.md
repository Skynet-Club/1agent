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
2. `1agent init` creates the home and `journals/`, `payloads/` and `backups/`
   with mode 0700. It is idempotent. A non-empty directory without the marker is
   refused instead of adopted, so a mistyped `--root` cannot overwrite user
   files.
3. `1agent doctor` reports actionable checks (marker/version, directory
   permissions, layout, master-key permissions, writability) and exits non-zero
   on a failing check. It does not repair state.
4. `1agent export` writes a directory bundle with an `export.json` manifest
   containing relative path, size and SHA-256 for every file. Key material and
   credential-looking files are excluded and listed under `excluded`. Files are
   written 0600 and the output must be empty and outside the home.
5. `1agent restore` verifies the manifest, rejects absolute/`..` paths, extra
   files, symlinks, key material and hash mismatches, and writes only into an
   empty or nonexistent home.
6. `1agent upgrade --check` reports missing layout without changing state.
   Upgrading repairs the current layout only. Any other home version is rejected
   until a real migration is registered; the version field is the migration
   hook.
7. `1agent uninstall` never deletes data without `--purge --yes`. Purge requires
   a valid home marker and refuses `/`, top-level paths, `$HOME` and a home that
   contains the current working directory.
8. Exports deliberately exclude the master key, so an export alone cannot
   decrypt memory payloads. Key escrow remains a separate product decision and
   is not claimed by this ADR.

## Consequences

- The product has a single, testable home lifecycle without adopting the key.
- Existing commands still accept explicit paths and relative defaults; moving
  every default onto the runtime home is incremental and must not silently
  reinterpret old data.
- `upgrade` currently has no historical version to migrate. S-Gate evidence for
  migration and rollback requires a future schema bump plus a registered
  migration, not a no-op claim.
