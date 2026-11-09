# Status v0.22.0

Date: 2026-11-02
Status: deterministic archives verified by consecutive local package runs

## Changes since v0.21.0

- `SOURCE_DATE_EPOCH` now controls the provenance generation timestamp.
- The package script removes stale package trees, tar.gz files and zip files before packing.
- `SHA256SUMS` is generated before timestamps are normalized.
- Tar output has sorted entries, zero ownership, a fixed mtime and deterministic gzip headers.
- Zip output has sorted entries, UTC timestamps and no directory entries.
- Added `scripts/verify-reproducible-package.sh` as an explicit release gate.

## Verified development result

The archive implementation was first exercised on the v0.21.0 source state. The initial zip
comparison failed because `SHA256SUMS` was timestamped after normalization and the existing zip
retained directory entries with changing timestamps.

After both causes were corrected, two consecutive full package runs produced:

```text
Linux tar.gz: c75a510e0451a71d6947c2d4aeb7cb39939b02b7c2bcf20fb89aff029460864c
Windows zip:  9babd6059ee7dc9d0e4bc43d36417c0fe59be854eec01d040e096ae95f111e64
Linux binary: c820bbd56ae2d0c339ddf02f5f508ca979476b30d090fc7fe0edcab8871e3ae5
Windows exe:  2695822629cc3936438b5b28e94790806aa8281ae05c1b2f3962ebcd50ba0f4c
```

The v0.22.0 release artifacts are rebuilt from the committed source state. Their hashes are
published in the release manifest and the Obsidian technical document, because a package cannot
contain a self-referential archive hash without changing that hash.

## Verification

| Check | Result |
|---|---|
| Linux binary double-build equality | passed |
| Windows binary double-build equality | passed |
| Linux archive consecutive equality | passed |
| Windows archive consecutive equality | passed |
| SHA-256 payload manifest | passed |
| Deterministic package gate | passed |
| Unit tests | 61 passed |
| Core evaluation rubric | 4/4 passed, 0 hard-gate failures |

## Requirement comparison

| Requirement | Status |
|---|---|
| Normalize archive timestamps | implemented |
| Normalize archive ownership | implemented |
| Stable file order | implemented |
| Stable gzip metadata | implemented |
| Remove stale archive entries | implemented |
| Consecutive local package equality | verified |
| Clean-source provenance | pending release rebuild |
| Cross-machine hermetic equality | not yet implemented |

## Known risks

- Byte equality is proven for consecutive runs in this workspace, not across machines.
- The toolchain, linker and registry are not hermetic.
- Provenance still records an absolute artifact path.
- Archive hashes cannot be embedded in the same immutable archive without recursion; the release
  manifest and external technical document carry those final digests.

## Next requirement

Treat distribution as a production concern:

1. Build from the committed release revision and require a clean provenance state.
2. Verify extracted archives rather than trusting archive bytes alone.
3. Publish a compact release manifest outside the archive.
4. Define cross-machine reconstruction and toolchain pinning.
5. Keep promotion closed until manifest, binaries and package contents agree.
