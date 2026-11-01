# Status v0.24.0

Date: 2026-11-02
Status: release-manifest approval and key-state enforcement implemented

## Changes since v0.23.0

- Added `release_manifest` to `ArtifactKind`.
- Added strict expected-kind validation so a valid approval for another artifact type cannot
  authorize a release manifest.
- `release verify` can now consume an approval envelope, key environment variable and approval
  key registry.
- Signed verification requires all three inputs together.
- The registry enforces active, retired and revoked key behavior:
  - active keys may sign new releases;
  - retired keys may verify historical releases but may not sign new ones;
  - revoked keys invalidate release verification.
- `approval sign --artifact-kind release_manifest` supports the existing key lifecycle.

## Verification

| Check | Result |
|---|---|
| Release approval signs and verifies | passed |
| Retired key verifies historical release | passed |
| Retired key cannot sign new release | passed |
| Revoked key invalidates release | passed |
| Wrong artifact kind rejected | passed |
| Unit tests | 66 passed |
| Core evaluation rubric | 4/4 passed, 0 hard-gate failures |

## Known limitations

- HMAC uses a shared symmetric key, so verifiers can also forge approvals.
- Registry distribution and trust-anchor rollout remain operator responsibilities.
- Signing is optional in the default local package command because no release key is available in
  this environment; a production release policy must require it.
- Cross-machine hermetic builds remain unverified.

## Next requirement

Add an explicit signing policy to the package gate:

1. Support unsigned development packages with a visible status.
2. Require a signed manifest for promotion.
3. Verify the registry snapshot/trust-anchor chain used for the signing key.
4. Make missing keys a hard failure in production mode.
5. Re-verify release artifacts after approval.
