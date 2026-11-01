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
| Archive extraction comparison | passed |

## Verified clean release

```text
source commit: 6e7a2f21680eeef0adca62d28a312da8af8ca221
git dirty: false
generated_unix_ms: 0

Linux binary:
d2c31969b587f33823c3c31efd8dec6c13328676bada9cddeb5a9d574bab6dc5

Windows binary:
d12a32e73dd1582a99c200502e49c8ff498eee8247b9528435077ea7e3b49945

Linux tar.gz:
775adc55c87602383d7d8cf52947dcbabd94dcffd14178ca711d73a6ba10a995

Windows zip:
f0b080ac7d204a89ba8cc45f7d2e4e7806f9ebe611dd047fffb29bf272ea4245

Release manifest:
f55ff53599f00e56391a80a66740ed70fa13eaa0429a94762d8bd01120427fdb
```

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
