# 1.0 platform code-gate evidence (2026-11-17)

Scope: code-level cross-platform checks, not GA. No real Windows or macOS host
was used. A cross-compiled `.exe` is not proof of Windows support; it is only a
compile/link gate.

## 1. Linux x86_64

- Host: Linux x86_64.
- `./scripts/verify.sh`: pass on the final code line (see the final verification
  log; 1632+ tests, core-v3 8/8).
- Track Q final full run: 99/100, 0 hard-gate, audit/replay 100/100.
- Python SDK: 6/6 unit tests.

## 2. Windows GNU

Command:

```bash
./scripts/cross-check.sh --build
```

Result: **pass**.

- `cargo check --offline --all-targets --target x86_64-pc-windows-gnu` with
  `-D warnings`: pass after fixing four cfg-specific warnings:
  - `browser/policy.rs`: `Write` import is Unix-only;
  - `tool_approval.rs`: `Write` import is Unix-only;
  - `browser/session.rs`: `pid` is only used in the Unix kill branch;
  - `onboarding.rs`: `OpenOptions` is only mutated on Unix.
- Release link: pass; binary:
  `target/x86_64-pc-windows-gnu/release/first-principles-agent.exe`
- SHA-256: `535eb26050b664796964ff4b3212ee56e5673799a2e207c9c405e3c0725b1d1e`
- Not tested: execution on Windows, ConPTY, Windows ACL/DPAPI, installer,
  Authenticode signing. **Windows GA is blocked.**

Raw log: `platform-1.0-2026-11-17-crosscheck.log`.

## 3. macOS arm64

Command target `aarch64-apple-darwin` is installed, but the check is blocked by
the host C toolchain while building `ring`:

```text
cc: error: unrecognized debug output level 'full'
cc: error: unrecognized command-line option '-arch'
cc: error: unrecognized command-line option '-mmacosx-version-min=11.0'
error: failed to run custom build command for `ring v0.17.14`
```

`cross-check.sh` records this as a host-toolchain block unless
`--require-macos` is passed. No macOS run, signing or notarization was done.
**macOS GA is blocked.**

## 4. Linux ARM64

No `aarch64-unknown-linux-gnu` target is installed on this host. Not tested;
mark `untested`.

## 5. Verdict

- Linux x86_64: local code gates verified.
- Windows GNU: compile and release link verified; runtime/signing/GA blocked.
- macOS: host-toolchain blocked; GA blocked.
- Linux ARM64: untested.
- P-Gate GA is **not passed**. Do not claim cross-platform GA from this file.
