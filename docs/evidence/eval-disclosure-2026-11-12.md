# Eval Disclosure Bridge — Evidence (2026-11-12)

Status: **passed on this machine**  
Scope: `1agent eval --disclosure-out` and its acceptance by
`1agent trust-center --evaluation`.

## Commands

```bash
cargo test --test eval_disclosure_cli
cargo clippy --all-targets -- -D warnings
```

## Expected results

```text
running 1 test (tests/eval_disclosure_cli.rs)
test eval_writes_a_trust_center_disclosure ... ok

test result: ok. 1 passed; 0 failed
```

## What this proves

- Running `evals/core.json` writes a disclosure whose name and passed/total
  match the eval report.
- The evidence digest is 64 hex characters.
- `trust-center --evaluation` accepts the file and lists one evaluation.

## What this does not prove

- That the eval ran against a real model or that its result generalizes.
- That the digest is anchored outside the local machine.
- That a third party can reproduce the run.
- Cross-platform behavior; tests ran on Linux only.

## Files

- `src/main.rs` (`eval --disclosure-out`)
- `tests/eval_disclosure_cli.rs`
- `docs/adr/0076-trust-disclosure-manifest.md`
- `docs/architecture.md`, `docs/features.md`, `docs/features.en.md`,
  `docs/claims.md`, `CHANGELOG.md`
