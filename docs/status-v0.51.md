# Status v0.51.0

Date: 2026-11-02
Status: conflict resolution provenance implemented

## Changes since v0.50.0

- Conflict records add `resolution_claim_ids`.
- Resolution source validation now requires each source to be declared by:
  - one of the two conflict-side claims; or
  - an explicit resolution claim.
- Resolution claims must exist and must not duplicate side claims.
- Unresolved conflicts keep empty resolution provenance.
- Provider contract and human schema documentation include resolution claims.

## Verification

| Check | Result |
|---|---|
| Side claim resolution source | passed |
| Undeclared resolution source | rejected |
| Resolution claim references | implemented |
| Unresolved provenance | retained |
| Unit tests | 87 passed |
| Core evaluation | 6/6 passed |

## Known limitations

- Resolution claims are structurally validated but not semantically judged.
- Source freshness/timestamp metadata is not implemented.
- Multi-party conflicts still need multiple two-side records.

## Next requirement

Add source freshness:

1. Add observation time and expiry metadata to evidence.
2. Reject expired evidence when task policy requires freshness.
3. Report stale source count.
4. Preserve unknown freshness explicitly.
5. Test serialization and evaluation gates.
