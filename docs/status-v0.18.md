# Status v0.18.0

Date: 2026-11-02
Status: external registry trust anchors

## Added since v0.17.0

- `TrustAnchorManifest`.
- `approval anchor`.
- Chain verification starts from a pinned first snapshot digest.
- Anchor binds operator and authorized admin key ids.
- Unrelated but internally valid histories are rejected.
- Unauthorized snapshot keys are rejected.
- Calibration requires a chain that begins at the supplied trust anchor.

## Verification

| Check | Result |
|---|---|
| Unit tests | 58 passed |
| Valid anchored chain | passed |
| Alternate-history rejection | passed |
| Unauthorized-key rejection | passed |
| Core evaluation rubric | 4/4 passed |
| Clippy `-D warnings` | passed |

Release artifacts:

```text
Linux binary:   c2756e5926513667f9e8942f191af139c16616ade32bff769cebf8024d8fa591
Windows binary: 2513f2edaa69e46576a43963a19c4ea61b9e0bec122e02121f9497dec129db6d
Linux tar.gz:   a28dfb8bfb782589e5ddaa8f410ef14985a97bae0a7cdfb3026fbc7e174b2727
Windows zip:    586977426dca4e7a81507133e78d2695c5c948eeed9d251d50676c893bf23c49
```

## Requirement comparison

| Requirement | Status |
|---|---|
| Pinned first snapshot | implemented |
| Operator/key authorization | implemented |
| Anchored chain verification | implemented |
| Alternative history rejection | implemented |
| Calibration anchor gate | implemented |
| Secure anchor distribution | operational, pending |
| Asymmetric signatures | pending |

## Known risks

- Trust anchor distribution is out-of-band and not automated.
- Anchor itself is unsigned; its integrity depends on deployment channels.
- HMAC remains symmetric.
- Operator identity is a local unauthenticated string.

## Next requirement

Add reproducible build provenance for release artifacts:

1. Record source commit and dirty-state status.
2. Record toolchain versions and target triples.
3. Hash executable and archive outputs.
4. Sign provenance with the approval envelope.
5. Rebuild and compare hashes in verification.
