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
| Clean-source package gate | passed |

## Verified clean release

```text
source commit: f9b14873a3a85ef055a750ed8894622edb8c2788
git dirty: false

Linux binary:
acdc073bfc0e7d573f22fb05720363ad84a3a6bacdb01b71a3f45aa068e042fd

Windows binary:
b0be928d2a6f5713b8f64f9a482565e10e18254e055d9fb1b52aa69362398135

Linux tar.gz:
5e445dae42d7122d0ec380f0dae0de5a05009c4eac3e8a4f14e98e6f4f0d0deb

Windows zip:
04cad0d443a08f0ff8b948b946c86ebe45ff16d202081ebebc48c6d24061b111

Release manifest:
53d5371c631f525d1fc108b4acfe3c82e6704f4087611ef62cdcbe3f45ebebef
```

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
