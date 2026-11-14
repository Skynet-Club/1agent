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
| Real CLI sign/verify lifecycle | passed with ephemeral test key |
| Clean-source package gate | passed |

## Verified clean release

```text
source commit: 7ea606e51c6068bab08b6a203064cb7f4951a4be
git dirty: false

Linux binary:
9e29ae5583bf8466cecf2230e9d5f90489f5c918f45ba10451a33a7d1a5eb377

Windows binary:
34caa9ef48dd5d77a4b6e7cc1be604102cbaa530f050f0408ac0e38cabd86d16

Linux tar.gz:
3c8065d7a0b622b08486d61b80624877afed363efe1baa28bd8cd4abb649fb55

Windows zip:
e8cbdcb26428df0cfb778359f8472fa687187a52bc7c43bf68ecf21e0889c050

Release manifest:
7a5ebe91e4a104d2ff57e5be88e31bc73b64cebc006e122241532a3ff1d6decd
```

The ephemeral approval only proves that the CLI and key-state lifecycle work. It is not a
production trust statement.

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
