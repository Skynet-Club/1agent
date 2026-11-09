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
| Clean-source package gate | passed |

## Release artifacts

Source commit: `db2327f09b880e8a8f79c3a87e601f43ba096890`

Clean-source artifacts were rebuilt after the feature commit. SHA-256:

| Artifact | SHA-256 |
|---|---|
| Linux `tar.gz` | `35c1277b445ad440171f50648a2a00cf3847ed008f08d8b0bbad0af0805a622f` |
| Windows `zip` | `146fa274956936f7f840f719cd2bd3c1082516d2c6f096c16e6415e400467680` |
| Release manifest | `f01d608cfc64bd5ec469cd29dc0b68f6b1a6c7b72073831b4d7414a5d2482f09` |
| Linux binary | `5726bf88f9a304ae3ad2a647afb27506416d8c10039bf27f60adf324452a45ba` |
| Windows binary | `791e247eb50ab1a28424ddafec6ac06f75fae5b882f87cefd49b6e574761e9b5` |

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
