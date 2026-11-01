# Status v0.71.0

Date: 2026-11-02
Status: experience lifecycle graph audit implemented

## Changes since v0.70.0

- Experience audit adds status counts for all lifecycle states.
- Audit adds:
  - supersession chain count;
  - maximum chain depth;
  - expired Active count.
- Orphaned supersession targets fail audit.
- Cyclic supersession chains fail audit.
- Expired Active experiences fail audit.
- New stable codes:
  - `orphaned_supersession`;
  - `supersession_cycle`;
  - `expired_active`.

## Verification

| Check | Result |
|---|---|
| Unit tests | 118 passed |
| Integration / E2E | passed offline |
| Core evaluation | 6/6 passed |
| status counts | passed |
| chain depth | passed |
| orphan reference | rejected |
| cyclic reference | rejected |
| expired Active | rejected |
| normal replacement chain | passed |
| Clippy `-D warnings` | pending |
| Clean-source package gate | pending |

## Known limitations

- Audit follows chains from every candidate, so shared history is counted repeatedly.
- Depth is bounded by candidate count but not separately configurable.
- Expiration uses current wall clock.
- Audit graph is derived from local experience events only.

## Next requirement

Summarize lifecycle findings in human-readable CLI output:

1. show state counts;
2. show chain count and depth;
3. show expired Active count;
4. keep JSON schema stable;
5. test clean and failing text output.
