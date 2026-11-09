# Status v0.26.0

Date: 2026-11-02
Status: versioned release policy and machine-readable verification report implemented

## Changes since v0.25.0

- Added `ReleasePolicy` schema version 1.
- Production policy binds:
  - minimum registry version;
  - allowed release-manifest signer key IDs;
  - allowed registry snapshot signer key IDs.
- Production policy rejects missing signer allowlists or a zero registry floor.
- Release verification checks the approval envelope key against the policy allowlist.
- Registry snapshot and trust-anchor key IDs are checked against the registry allowlist before
  chain verification.
- `release verify --json` returns a structured report containing package version, commit,
  manifest digest, policy mode, signature/chain status and verified artifact count.
- Added `configs/release-policy.example.json`.

## Verification

| Check | Result |
|---|---|
| Development policy without signature | expected |
| Production policy validation | passed |
| Manifest signer allowlist | passed |
| Registry signer allowlist | passed |
| Invalid production policy | rejected |
| Policy-based CLI verification | passed |
| Unauthorized manifest signer | rejected |
| JSON verification report | passed |
| Unit tests | 67 passed |
| Core evaluation rubric | 4/4 passed, 0 hard-gate failures |
| Clean-source package gate | passed |

## Verified clean release

```text
source commit: 33ea9a4d783495ee556f4af19c19ac08e4eb5082
git dirty: false

Linux binary:
910e0ddd55b82b2df3f818d53b58685bc2af680022bf02cd65a20fa3bb67a821

Windows binary:
bff2f284f342d6836808b97116bc06e716ed75e733ee7de16f07a63abc1b5373

Linux tar.gz:
85ef5fd4f0494afba02d49ca3d94815c221448d449ae27cac3659dccf3eecd70

Windows zip:
62142eb206d3bbf31fefe96b34c920c20e24afd2b5a143c90820f765ce36cacf

Release manifest:
ab8894a05c7ecae12cc82bcfe8eb7ef8526b1af113a270a18e6eeb9d60e571a3
```

## Known limitations

- Policy files are not yet signed; their trusted distribution remains an operator concern.
- Signer values are key IDs, not authenticated role identities.
- HMAC remains symmetric.
- Cross-machine hermetic builds and Windows runtime execution remain unverified.

## Next requirement

Apply the policy to package promotion:

1. Keep local package generation in development mode.
2. Add a separate promotion command that consumes policy, approval and snapshot chain.
3. Make promotion output the only source of a production-ready status.
4. Add automated negative cases for wrong anchor, fork, rollback and missing snapshots.
5. Preserve immutable release evidence for every promotion decision.
