# Status v0.41.0

Date: 2026-11-02
Status: host-visible tool capability inventory implemented

## Changes since v0.40.0

- Added `tools` and `tools --json`.
- Reports:
  - tool name;
  - description;
  - JSON Schema;
  - risk;
  - write requirement;
  - timeout;
  - capability status.
- Capability statuses are:
  - implemented;
  - degraded;
  - unsupported.
- Current built-ins report `implemented`.
- Inventory is generated from the runtime ToolRegistry rather than duplicated docs.
- Output contains no configured filesystem paths or secrets.

## Verification

| Check | Result |
|---|---|
| read_file advertised | passed |
| search_files advertised | passed |
| Runtime/spec source shared | enforced |
| Schema output | passed |
| Path redaction | passed |
| Unit tests | 80 passed |
| Core evaluation rubric | 4/4 passed, 0 hard-gate failures |
| Clean-source package gate | passed |

## Verified clean release

```text
source commit: c10056dda75d8c193dc0111289c759b32620478c
git dirty: false

Linux binary:
5e86244ffdb1a3d71c4725358eb666b910155b82c0f3edaf078a7f8d58d866eb

Windows binary:
714226428d6f0e91737f0fd98966a391fa1f46b105a071307f97406d32331f4e

Linux tar.gz:
40e94bd7c2b903241548d2e68724e6e7b044d597fc706515f31ba506c4e95843

Windows zip:
5ff330a87d108863251c2beae65d8b85df8384a42dc84e5407d04d58f5714fbb

Release manifest:
0aad02a1b7fd234f4bd9a40ffc8a30a6f7c4ecc87c339d37220d214590700a77
```

## Known limitations

- The status taxonomy is present, but no current tool is degraded or unsupported.
- Platform-specific capability probing remains future work.
- Tool inventory does not include external process/network tools because they are not implemented.

## Next requirement

Bind run outputs to a tool capability snapshot:

1. Journal a compact digest of advertised tool schemas.
2. Compare the snapshot during resume.
3. Reject resume when tool schemas or capabilities change incompatibly.
4. Keep full schema inventory outside every event.
5. Preserve legacy resume behavior.
