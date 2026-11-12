# Eight-Case Repeatable Core Suite — Evidence (2026-11-12)

Status: **passed on this machine**
Scope: `core-v3` contract suite and repeatability check.

## Commands

```bash
cargo run --offline --quiet -- eval --suite evals/core.json
cargo test --test eval_repeatability_cli
```

## Result

```text
评测：core-v3（ContractFixture），通过 8/8，硬门禁失败 0，rubric 6/6
- read-evidence: PASS
- needs-authorization: PASS
- honest-without-reasoning-provider: PASS
- hidden-control-rejected: PASS
- missing-file: PASS
- zero-width-control-rejected: PASS
- read-directory-rejected: PASS
- sleep-then-honest: PASS
```

The repeatability test runs the suite twice and compares a verdict projection
for all eight cases; both runs are 8/8 with zero hard-gate failures.

## What this proves

- The default offline gate covers eight deterministic cases.
- A directory read path fails closed with `tool_failed` and an honest
  `needs_evidence` answer, with a checkpoint.
- The sleep path returns the honest answer without a tool call.
- Two runs agree on every verdict and invariant.

## What this does not prove

- Model quality or real-task success; the suite is `contract_fixture`.
- Byte-identical reports: `content_sha256` includes per-case `duration_ms`, so
  the digests differ between runs.
- That eight real tasks exist; a real benchmark still needs a provider and
  reviewed tasks.
