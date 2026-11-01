# Status v0.53.0

Date: 2026-11-02
Status: versioned answer contract implemented

## Changes since v0.52.0

- Structured answers now require `schema_version: 1`.
- Legacy answers without a version decode as version 1 and remain valid.
- Unknown future versions fail closed before semantic answer validation.
- Journal audit reports the observed answer schema version.
- The provider prompt, deterministic provider and test fixtures emit versioned answers.
- CLI audit output shows the answer contract version.

## Verification

| Check | Result |
|---|---|
| Version 1 accepted | passed |
| Legacy JSON defaults to version 1 | passed |
| Version 2 rejected | passed |
| Version included in serialization | passed |
| Version included in journal audit | passed |
| Unit tests | 92 passed |
| Core evaluation | 6/6 passed |
| Clean-source package gate | passed |

## Compatibility

Unversioned answers from earlier journals remain readable; their in-memory version is 1.
Nothing infers compatibility for later contract versions. A future version must be implemented
and tested explicitly rather than accepted through permissive decoding.

## Verified clean release

```text
source commit: e0a3d3d958ce06926fca13835dfc76ec387f5817
git dirty: false

Linux binary:
e06b05103a206d6041a12877be0536b03f8d04100dbcc838f08a54938d755f39

Windows binary:
f156a4fbd861581dc150806da7c5a8b396765d7af91c58b69f7ab7d06a95c5b0

Linux tar.gz:
09fea19414a4e949aaad658a9def41179a6aeeae2ef58811a96481fccd3e5159

Windows zip:
c55ae662f763b694fa64d02ed8c9fb01fd915e68a92aac2a122f4f8c08794bc2

Release manifest:
0b361153869a4411707d68f5fbc38d943cc24b2eb60f795ec2b17ced7716460c
```

## Known limitations

- The version identifies the answer envelope, not every nested record independently.
- No migration command rewrites historical journal files.
- Provider output remains untrusted until decoded and validated.

## Next requirement

Make journal audit verify answer-version consistency across the full answer lifecycle:

1. Reject answers whose journal representation is ambiguous or unsupported.
2. Preserve the schema version through replay.
3. Surface version mismatch as a stable audit finding.
4. Test legacy journals and unsupported-version journals.
