# Actionable Error Recovery — Evidence (2026-11-12)

Status: **passed on this machine**
Scope: `RuntimeError::recovery_hint` and the `run`/`resume` CLI output.

## Commands

```bash
cargo test runtime_errors_carry_actionable_hints --lib
cargo test --test cli_e2e provider_network_failure_prints_a_recovery_hint
```

## What this proves

- Authentication errors hint at checking the API key/account; network errors
  hint at connectivity/proxy and `--timeout-seconds`.
- Cancellation hints at `resume`; a resume failure hints at
  `resume-status`; a journal error hints at writability/space.
- A CLI run against an unreachable loopback provider exits non-zero and
  prints `hint:` on stderr alongside the original error.

## What this does not prove

- That the hint resolves the failure; it is static advice and does not retry,
  switch providers or change permissions.
- That the TUI shows the hint; only `run` and `resume` print it.
- That every provider-specific message is covered; unknown codes map to the
  nearest category.
- That a user can always act on the hint in a sandboxed environment.
