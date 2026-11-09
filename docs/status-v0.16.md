# Status v0.16.0

Date: 2026-11-02
Status: signed approval-registry snapshots

## Added since v0.15.0

- Monotonic registry version increments on key changes.
- `approval snapshot`.
- `approval verify-snapshot`.
- Snapshot binds registry SHA-256, version, approver, admin key id and MAC.
- Verification requires a trusted minimum registry version.
- Rollback below the minimum is rejected.
- Mutation after snapshot is rejected by artifact digest mismatch.
- Calibration verifies both registry snapshot and artifact approval.

## Verification

| Check | Result |
|---|---|
| Unit tests | 56 passed |
| Signed registry snapshot | passed |
| Post-snapshot mutation | passed |
| Version rollback rejection | passed |
| Core evaluation rubric | 4/4 passed |
| Clippy `-D warnings` | passed |

Release artifacts:

```text
Linux binary:   5236c5a76f3df508dde7f747a86e22f7a74af0313b7ceca2fd03e8c47a081a8b
Windows binary: 02452b8c0461bc3557929ba64cfe9ef4ecba41e7807915e42eb55cd22359d3c3
Linux tar.gz:   06454db33c6df8bed4c7ce1de808794228f2feadcea03f9973a2397846111ab2
Windows zip:    ce22d64f46d0382fedcb458991318d1c585f0b5cffc9cccaaecb089fc83cb851
```

## Requirement comparison

| Requirement | Status |
|---|---|
| Signed registry snapshot | implemented |
| Registry digest binding | implemented |
| Monotonic version | implemented |
| Rollback floor | implemented |
| Snapshot validation in calibration | implemented |
| Trusted timestamp | pending |
| Asymmetric signatures | pending |

## Known risks

- Minimum version is supplied by the caller and must itself be protected operationally.
- HMAC remains symmetric.
- Snapshot history is not chained; only a caller-provided rollback floor is enforced.
- No automatic key-service integration exists.

## Next requirement

Add a hash-chained registry snapshot history:

1. Each snapshot references the previous snapshot digest.
2. Verification requires a contiguous valid chain.
3. Forked and out-of-order histories are rejected.
4. Calibration consumes only the verified chain tip.
5. Add ordering/fork tests.
