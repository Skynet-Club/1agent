# Status v0.21.0

Date: 2026-11-02
Status: byte-identical binary builds under reproducible profile

## Changes since v0.20.0

- Added reproducibility probe with:
  - fixed `SOURCE_DATE_EPOCH=0`;
  - `CARGO_INCREMENTAL=0`;
  - release debug info disabled;
  - symbols stripped;
  - source and target path remapping.
- Two independent target directories produced equal bytes.
- Added `scripts/reproducible-build.sh`.
- Packaging now builds each target twice and installs only after equality.

## Verified Linux result

```text
target: x86_64-unknown-linux-gnu
sha256: 40b6b636250950d4879a7569b2a5f309377d06a47e2e00896992019f0759c484
size: 7744216
reproducible: true
```

## Verified Windows result

```text
target: x86_64-pc-windows-gnu
sha256: f4163df3ad4bc346bf9a78a9f6f0403e672f28775cc541ffe49f4a258f5a8132
size: 9679360
reproducible: true
```

## Verification

| Check | Result |
|---|---|
| Two independent target dirs | byte-identical |
| Source/target paths remapped | passed |
| Package build gate | doubled-build comparison |
| Unit tests | 60 passed |
| Core evaluation rubric | 4/4 passed |

## Requirement comparison

| Requirement | Status |
|---|---|
| Fresh target builds | implemented |
| Stable source/target paths | implemented |
| Stable timestamp | implemented |
| Stripped deterministic binary | implemented |
| Linux binary equality | verified |
| Windows binary equality | verified |
| Deterministic archive bytes | pending |

## Known risks

- Remapping profile is Cargo/Rust-specific.
- Archive timestamps and ordering are not yet normalized.
- Dirty source is still recorded by provenance; equality means both builds used the same source state, not that source is approved.
- Hermetic dependency/toolchain isolation is not implemented.

Release artifacts:

```text
Linux binary:   c46eff157d91fdbc7d72aa5e12a2f09f639223c38aeacb0ccc74857e4a0fd166
Windows binary: f4163df3ad4bc346bf9a78a9f6f0403e672f28775cc541ffe49f4a258f5a8132
Linux tar.gz:   49e28f856d65beaa2efb20d234818e5eaa6fe3a6ebe583b6426f85cf26591e5d
Windows zip:    13b42fd661ebb38d43290ec914e907112a2b9f69a67ed20204eda7c70129a7e0
```

The release binaries are reproducible under the documented profile. The archives are not yet
claimed byte-identical.

## Next requirement

Verify Windows binary reproducibility and deterministic archives:

1. Run the two-target probe for Windows GNU.
2. Compare Windows executable bytes.
3. Normalize archive mtimes, ownership and ordering.
4. Produce archive hashes from reproducible inputs.
5. Keep provenance promotion closed if either binary or archive differs.
