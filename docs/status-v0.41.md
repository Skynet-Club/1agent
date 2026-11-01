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
