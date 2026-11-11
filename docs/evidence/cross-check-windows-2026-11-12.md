# Windows Cross-Check — Evidence (2026-11-12)

Status: **Windows GNU check passed; macOS check blocked by host toolchain**  
Scope: cross-target compilation of the current tree, not runtime behavior.

## Commands

```bash
RUSTFLAGS="-D warnings" cargo check --offline --target x86_64-pc-windows-gnu
cargo check --offline --target aarch64-apple-darwin
cargo test --offline
```

## Results

Windows GNU target:

```text
Checking first-principles-agent v0.96.1
Finished `dev` profile [unoptimized + debuginfo] target(s) in 20.17s
```

No warnings under `-D warnings`. One cfg-only unused-variable warning was
fixed: `read_key_hex(path, secret)` now consumes `secret` on non-Unix targets.

macOS arm64 target:

```text
error occurred in cc-rs: ... "-arch" "arm64" ... ring-0.17.14/crypto/curve25519/curve25519.c
```

The failure is the host C toolchain lacking a macOS SDK/arm64 compiler for
`ring`'s C code, not a Rust type or API error in this repository. It is not
counted as a macOS pass.

Linux full tests after the change: 954 passed, 0 failed, 5 ignored.

## What this proves

- The Rust tree compiles for `x86_64-pc-windows-gnu` with warnings denied.
- Non-Unix cfg branches do not reference Unix-only APIs in a way that breaks
  compilation.
- The Linux suite still passes.

## What this does not prove

- That the Windows binary links, installs or runs.
- PTY/ConPTY, path, permission or signal behavior on Windows.
- That macOS compiles or runs; the host toolchain blocked the check.
- Installer, signing, notarization or CI matrix execution.

## Files

- `src/main.rs` (`read_key_hex` non-Unix handling)
- `docs/evidence/cross-check-windows-2026-11-12.md`
- `CHANGELOG.md`, `docs/claims.md`
