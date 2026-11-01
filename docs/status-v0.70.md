# Status v0.70.0

Date: 2026-11-02
Status: machine-readable experience audit implemented

## Changes since v0.69.0

- `ExperienceAuditReport` adds:
  - schema version;
  - ok;
  - event count;
  - candidate count;
  - structured findings.
- Stable finding codes classify I/O, JSON, schema, sequence, size, timestamp,
  candidate, transition, evidence, metadata and supersession failures.
- CLI adds `experience audit --store ... [--json]`.
- Clean stores report zero findings.
- Corrupt stores remain diagnostic without mutating state.

## Verification

| Check | Result |
|---|---|
| Unit tests | 116 passed |
| Integration / E2E | passed offline |
| Core evaluation | 6/6 passed |
| clean audit report | passed |
| corrupt store finding | passed |
| `event_too_large` serialization | passed |
| CLI JSON audit | passed |
| candidate list compatibility | passed |
| Clippy `-D warnings` | pending |
| Clean-source package gate | pending |

## Known limitations

- Audit reports the first replay error, not every independent issue.
- A corrupted oversized line is allocated before size rejection.
- Audit report itself is unsigned.
- Audit does not query source journals for digest revalidation.

## Next requirement

Audit experience lifecycle graph summaries:

1. report candidate status counts;
2. report supersession chain depth;
3. detect orphaned or cyclic supersession references;
4. report expired Active candidates;
5. keep replay fail-closed.
