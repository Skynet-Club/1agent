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
