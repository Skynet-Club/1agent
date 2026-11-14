# ADR 0120: GitHub CI Matrix for Local and Cross-Platform Checks

- Status: accepted
- Date: 2026-11-12
- Related: SA97-39, SA97-07

## Context

The repository had no `.github/workflows`; the only cross-platform evidence was
a local Windows GNU `cargo check` and a macOS cross-build blocked by the host C
toolchain. SA97-39 asks for a CI matrix, and SA97-07 asks for hard gates to be
wired into CI/release checks.

## Decision

1. `.github/workflows/ci.yml` runs on push to `main`, pull requests and manual
   dispatch, with read-only repository permissions and concurrency cancellation.
2. Linux runs the existing local software gates with `./scripts/verify.sh`
   after `cargo fetch --locked`. This includes fmt, clippy, the full test
   suite, the core eval suite and the structural claims check.
3. Windows and macOS run `cargo check --locked --all-targets` and
   `cargo test --locked --no-run` after fetching dependencies. These are
   compile-level gates, not runtime/PTY/installer evidence.
4. No secrets are used; the workflow only builds and tests.
5. The local `scripts/cross-check.sh` remains the Windows GNU cross-build
   evidence until the workflow runs on GitHub.

## Consequences

- A push or PR now has a defined Linux gate and a compile check for the other
  two desktop platforms.
- Windows/macOS runtime behavior, ConPTY, file permissions and installers are
  still unproven; a green compile job is not P-Gate.
- The workflow has not been executed on GitHub yet; it is validated locally
  (YAML structure and the same cargo commands).
- The macOS job may still hit native build issues; a real macOS runner is
  required before claiming P-Gate.
