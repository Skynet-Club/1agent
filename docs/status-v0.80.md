# Status v0.80.0

Date: 2026-11-02
Status: versioned evaluation summary contract implemented

## Summary schema

New summaries use `schema_version: 1`. Version-less summaries decode as version `0` for
compatibility with v0.79 output.

Validation requires:

- `passed + failed == total`;
- `hard_gate_failures <= failed`;
- `decision_completeness_cases == total`;
- `decision_completeness_ok_cases <= decision_completeness_cases`;
- both basis-point fields are at most `10000`;
- the completion rate matches the case counts;
- the floor status matches the effective rate and floor;
- policy metadata is supported and consistent.

Unsupported versions and invalid summaries fail before output.

## Verification

| Check | Result |
|---|---|
| Unit tests | 123 passed |
| Integration / E2E | passed offline |
| Core evaluation | 6/6 passed |
| versioned summary output | passed |
| legacy version-less decoding | passed |
| unsupported version | rejected |
| basis-point overflow | rejected |
| count inconsistency | rejected |
| Clippy `-D warnings` | passed |
| Clean-source package gate | pending |

## Known limitations

- Summary validation checks arithmetic and metadata, not evaluation truth.
- Version `0` has the same field semantics as version `1` but no explicit version marker.
- Summaries remain unsigned.
- The summary intentionally omits failing case identities.

## Next requirement

Include a stable report-content digest in full inspection and summary output:

1. hash the normalized report payload without the digest field;
2. expose the digest in summary metadata;
3. recompute and reject mismatches on report inspection;
4. preserve compatibility with reports that predate the digest.
