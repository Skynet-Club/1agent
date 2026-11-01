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
| Core evaluation | pending |
| Clean-source package gate | pending |

## Compatibility

Unversioned answers from earlier journals remain readable; their in-memory version is 1.
Nothing infers compatibility for later contract versions. A future version must be implemented
and tested explicitly rather than accepted through permissive decoding.

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
