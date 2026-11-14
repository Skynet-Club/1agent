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
| Clippy `-D warnings` | passed |
| Clean-source package gate | passed |

## Known limitations

- Audit follows chains from every candidate, so shared history is counted repeatedly.
- Depth is bounded by candidate count but not separately configurable.
- Expiration uses current wall clock.
- Audit graph is derived from local experience events only.

## Verified clean release

```text
source commit: 086d9448157bf92f7d13e011ddc49b4d1a7d9e9c
git dirty: false

Linux binary:
b222b061c4ff98a7b563ef2bc525f7e6c45b6763d8d70f10cc64182650058193

Windows binary:
154c5d2a7c119379c645b6a71ae524aaa2becda54dbdac2ccc61a5748a420c36

Linux tar.gz:
f797c11b7d086851d98c21c3784924b870b20d321d4e075680e6a1c6eb7a90ed

Windows zip:
007764b50160431e900b7046930a360c5b1dfd35132966d3125d86dbaed9197f

Release manifest:
9585d1a158cdd02e9b3662698e21b206bb4e62bdda21347e567df49d2a678111
```

## Next requirement

Summarize lifecycle findings in human-readable CLI output:

1. show state counts;
2. show chain count and depth;
3. show expired Active count;
4. keep JSON schema stable;
5. test clean and failing text output.
