# Status v0.20.0

Date: 2026-11-02
Status: reproducible-build comparison implemented; current build fails byte equality

## Added since v0.19.0

- `provenance rebuild`.
- Fresh temporary target directory.
- Offline release build for the requested target triple.
- Artifact SHA-256 and size comparison.
- Nonzero exit when hashes or sizes differ.

## Measured result

Linux target:

```text
target: x86_64-unknown-linux-gnu
original hash: b8ffd690661678ac2eb9e4193f2a9a8c5e49213b6b2d2589aa6736007470c0b3
rebuilt hash:  774e4f5dd2aaa12e84191479d8aa19f956336ebc989bfe7d135cb2732f2d5dfe
original size: 10561800
rebuilt size:  10565640
hashes_equal: false
sizes_equal: false
```

This is a failed reproducibility result, not a pass. The current build remains a development
artifact and must not be promoted as byte-for-byte reproducible.

## Verification

| Check | Result |
|---|---|
| Unit tests | 60 passed |
| Artifact comparison helper | passed |
| Fresh-target rebuild executes | passed |
| Current Linux byte equality | failed as recorded |
| Core evaluation rubric | 4/4 passed |
| Clippy `-D warnings` | passed |

Release artifacts:

```text
Linux binary:   65481a3d26c7fb2fbac16a21a22d8689ac8b147282235825bbedd5d6f9a69e7c
Windows binary: 79075f57d0995367c4a45cc26d009176171a729c806f37f7fcdea0219088cf7c
Linux tar.gz:   4f79bb9e2bcea0d1bb493a7db3049906e693c64eaca819a0ed724d211d507360
Windows zip:    711344f30a4677b531e26f7d4fca0cfd52f77de9229c18d8216d58a4119329ee
```

These are development artifacts; provenance matches the files, but reproducibility is still false.

## Requirement comparison

| Requirement | Status |
|---|---|
| Fresh-target rebuild | implemented |
| Hash/size comparison | implemented |
| Fail-closed promotion gate | implemented |
| Current Linux reproducible result | failed |
| Hermetic toolchain/linker | pending |
| Path remapping / deterministic build flags | pending |

## Likely causes

- Build absolute path is embedded in debug/build metadata.
- Linker/build ID or Cargo metadata differs with target directory.
- Dirty worktree changes are present.
- Toolchain/linker flags are not hermetic.

These are hypotheses until isolated experimentally.

## Next requirement

Isolate reproducibility differences:

1. Compare a clean committed tree, not a dirty worktree.
2. Test fixed `CARGO_TARGET_DIR` and path remapping.
3. Compare build IDs and embedded paths.
4. Separate Linux and Windows reproducibility.
5. Keep the promotion gate closed until byte equality is proven.
