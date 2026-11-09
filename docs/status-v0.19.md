# Status v0.19.0

Date: 2026-11-02
Status: release provenance

## Added since v0.18.0

- `BuildProvenance` record.
- Fields:
  - package name/version;
  - git commit and dirty state;
  - rustc/cargo versions;
  - target triple;
  - artifact path, size and SHA-256;
  - generation time.
- `provenance create`.
- `provenance verify`.
- Optional HMAC approval of provenance.
- Package script emits provenance for Linux and Windows binaries.

## Verification

| Check | Result |
|---|---|
| Unit tests | 59 passed |
| Provenance artifact matching | passed |
| Artifact-change rejection | passed |
| Core evaluation rubric | 4/4 passed |
| Clippy `-D warnings` | passed |

Release artifacts:

```text
Linux binary:   3bb463da135a7651ab206b1f049be37359d9aff7601c0a0377ef24a71edac40b
Windows binary: f2d0cb039b1c903a1731df53170b234fc8c19c5e88bf814c05d4fa0038a88a60
Linux tar.gz:   6a2cc11f4ecbed2a7bb4af4f0cbe0b7ee9f79b052fcf09e1a171a02750b7cf59
Windows zip:    cee6ffa525297a5c8b7592c7b75b09b04b5e67c360c199f8a6e4216ed1e29070

Provenance reports are included under provenance/.
```

## Requirement comparison

| Requirement | Status |
|---|---|
| Commit and dirty state | implemented |
| Toolchain and target | implemented |
| Artifact hashes | implemented |
| Provenance signing | implemented optional |
| Reproducible byte-for-byte rebuild | pending |
| Hermetic build environment | pending |

## Known risks

- Dirty worktrees are recorded honestly but cannot prove release reproducibility.
- Rust/Cargo versions do not capture all dependency and linker versions.
- Absolute artifact paths may not be portable.
- Byte-for-byte rebuild comparison is not automated.

## Next requirement

Add reproducible-build comparison:

1. Rebuild the same target in a fresh target directory.
2. Compare artifact SHA-256 and size.
3. Refuse provenance promotion when hashes differ.
4. Record differences in toolchain or source state.
5. Keep hermetic environment work separate from deterministic comparison.
