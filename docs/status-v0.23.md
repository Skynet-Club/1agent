# Status v0.23.0

Date: 2026-11-02
Status: external release manifest and extracted-content verification implemented

## Changes since v0.22.0

- Added `ReleaseManifest` schema version 1.
- Bound package name/version, clean Git commit, rustc/cargo versions and generation timestamp.
- Required exactly one artifact for each release role:
  - Linux binary;
  - Windows binary;
  - Linux provenance;
  - Windows provenance;
  - package checksums;
  - Linux tar.gz;
  - Windows zip.
- Stored artifact paths relative to the external manifest and rejected absolute paths or parent
  traversal.
- Rejected dirty releases, duplicate roles/paths, missing roles, target/version mismatch and
  artifact digest or size drift.
- Required package `SHA256SUMS` to cover every file in the package.
- Added `release manifest` and `release verify`.
- Packaging now emits `dist/1agent-<version>-release.json`.
- The package gate compares the manifest as well as both archives, then extracts both archives,
  verifies checksums/provenance and compares their contents.

## Verification

| Check | Result |
|---|---|
| Manifest generation on clean v0.22.0 package | passed |
| Manifest verification | passed |
| Required artifact roles | 7 |
| Unit tests | 65 passed |
| Core evaluation rubric | 4/4 passed, 0 hard-gate failures |
| Archive extraction comparison | pending release run |

## Known limitations

- Manifest is not yet signed.
- Signing is separate from artifact binding and will use the existing approval key lifecycle.
- The verifier validates archives only by digest; the release gate performs extraction.
- Cross-machine hermetic reconstruction remains separate work.

## Next requirement

Bind the release manifest to the existing approval system:

1. Sign the manifest as an approval artifact kind.
2. Verify manifest approval before accepting release checksums.
3. Exercise key rotation and revocation against release approval.
4. Keep publishing closed when the signature, key state or manifest digest is invalid.
