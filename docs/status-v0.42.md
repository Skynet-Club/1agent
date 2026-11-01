# Status v0.42.0

Date: 2026-11-02
Status: tool capability snapshots bound to runs and checkpoints

## Changes since v0.41.0

- Added a compact `ToolCapabilitySnapshot` with:
  - schema version;
  - SHA-256 of the complete capability/schema inventory;
  - sorted tool names.
- The snapshot is journaled before execution.
- Replay exposes the advertised snapshot.
- New checkpoints store the snapshot digest.
- Resume recomputes the current snapshot and rejects incompatible tool changes.
- Legacy checkpoints without tool snapshots remain resumable.
- Resume-status exposes the checkpoint tool snapshot digest.

## Verification

| Check | Result |
|---|---|
| Runtime snapshot | 2 read-only tools |
| Snapshot digest | deterministic |
| Journal persistence | passed |
| Replay restoration | passed |
| Changed registry resume | rejected |
| Legacy checkpoint decode | passed |
| Unit tests | 80 passed |
| Core evaluation rubric | 4/4 passed, 0 hard-gate failures |

## Known limitations

- The snapshot hash covers descriptions, schemas and capability metadata, not tool implementation
  binary code.
- Runtime and resume must use compatible registries.
- No platform-specific capability probing exists yet.

## Next requirement

Add run-level public surface verification:

1. Verify command help and JSON outputs remain parseable.
2. Add a small contract test for every public CLI command.
3. Keep network commands explicitly opt-in.
4. Document stable versus experimental commands.
5. Avoid locking human text wording beyond summaries.
