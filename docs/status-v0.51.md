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
| Clean-source package gate | passed |

## Verified clean release

```text
source commit: 5a1940790b60e0d8e3cd41f292df23ac1ba59299
git dirty: false

Linux binary:
08a192899c51d114c9672e5d6478f9a718bfe9e936a1c9f9dc2677b648a8634c

Windows binary:
3a32258627278b738c6d8956a72f28db72e520942f0f30ceaf4391a0c1bb862b

Linux tar.gz:
c03870ed50fffa3b323a8bb5efbf67f8a2a209a7e0aa8f274576a451ca91bb7c

Windows zip:
d9893b246a10fe5bdbb227f83e3e64482b4e8e59a532e6fc1267236ef7de83aa

Release manifest:
bb1f012652b9f623a4ed1ee1f12f4b51c75baac4af9ce4448e20883f7484f82c
```

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
