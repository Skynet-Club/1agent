# Status v0.17.0

Date: 2026-11-02
Status: hash-chained registry snapshots

## Added since v0.16.0

- Registry snapshots have a self-digest.
- Each snapshot references the previous snapshot digest.
- `approval verify-chain`.
- Registry version must increase monotonically through the chain.
- Forked previous links are rejected.
- Out-of-order and duplicate-version histories are rejected.
- Only the final snapshot must match current registry bytes/version.
- Calibration consumes a verified chain and uses its tip.

## Verification

| Check | Result |
|---|---|
| Unit tests | 57 passed |
| Valid chain verification | passed |
| Out-of-order rejection | passed |
| Fork rejection | passed |
| Core evaluation rubric | 4/4 passed |
| Clippy `-D warnings` | passed |

Release artifacts:

```text
Linux binary:   c3704aab48f2a1844e5d74a2c62ce637fc72377b24dd437a6e9333c76b484012
Windows binary: 326ad65b62267a0f2c7fc3cb598faf365e39888320c11c8bdbcc2f272788de03
Linux tar.gz:   0bb55cf9916255125e6d931887778dc14cafc40d6ac8865f43d90132d4c0a087
Windows zip:    ee37360099a7852d558f61b6ca051e448190a0876b85453b6796bce17d659e1a
```

## Requirement comparison

| Requirement | Status |
|---|---|
| Snapshot chaining | implemented |
| Contiguous verification | implemented |
| Fork detection | implemented |
| Ordering detection | implemented |
| Chain-tip calibration | implemented |
| Trusted external anchor | pending |
| Asymmetric signatures | pending |

## Known risks

- The first snapshot still needs an out-of-band trusted digest.
- HMAC remains symmetric.
- Chain storage and pruning policy are local.
- Registry rollback is detected only relative to supplied history/minimum version.

## Next requirement

Add an external trust anchor manifest:

1. Pin the first registry snapshot digest.
2. Bind trust anchor to operator identity and artifact signature.
3. Require chain verification to start from the pinned anchor.
4. Reject unrelated chains with valid internal signatures.
5. Test anchor mismatch and alternate-history rejection.
