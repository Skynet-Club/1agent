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
| Clippy `-D warnings` | passed |
| Clean-source package gate | passed |

## Known limitations

- Audit reports the first replay error, not every independent issue.
- A corrupted oversized line is allocated before size rejection.
- Audit report itself is unsigned.
- Audit does not query source journals for digest revalidation.

## Verified clean release

```text
source commit: 66def581cd461395770be078787a8397ceffd915
git dirty: false

Linux binary:
d7067564589ed13caab2095a030c471a9a47f91f4509e4ed3b011984895ac871

Windows binary:
6da85215da7cdc9e48555823b7949b7b2f7d455f140b39007753e0ac764618c3

Linux tar.gz:
c1a9a9b6ac745da1722b30f59ec323a6dbfce04956a610fc3e611e02b5fe8c87

Windows zip:
392fa16ed10e80d0792e71ae319b5f9ce2e5b6b7e42266e852014d7f9b0fb3f1

Release manifest:
fb4f5e4ba3cfda881703c57ad20ce2bcee0c1a5bdc0528ff6641fe63c3588c80
```

## Next requirement

Audit experience lifecycle graph summaries:

1. report candidate status counts;
2. report supersession chain depth;
3. detect orphaned or cyclic supersession references;
4. report expired Active candidates;
5. keep replay fail-closed.
