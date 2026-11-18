# ADR 0149: Standard harness profile, not a benchmark platform

- Status: proposed
- Date: 2026-11-18
- Related: 1.1 standard harness plan

## Context

1Agent has a working runtime harness: model/tool loop, permissions, budgets,
journal/replay, artifact verification and local evaluation. External trust is
limited because the evidence is self-produced. Frontier practice shows that
agent results depend on the harness as much as the model, and that benchmark
validity is fragile:

- Harness-Bench reports large model–harness interaction effects.
- TraceTrial-Bench finds task success can coexist with harness-criterion
  failure.
- ABC identifies outcome/challenge/reporting validity gaps in popular
  agentic benchmarks.
- METR moved from its own Task Standard to Inspect.
- Inspect, Harbor/Terminal-Bench and A2A/MCP/OTel already provide reusable
  task, sandbox and interop layers.

## Decision

Build **1ASH**, a versioned, externally runnable evidence profile around the
existing 1Agent harness:

1. Task/run/trace/report schemas, reviewed like code.
2. Inspect AI as the v0 runner and sandbox substrate; Harbor/Terminal-Bench as
   an optional container backend; native `1agent run` as a subject adapter.
3. Trace IR built from the 1Agent journal, normalized to OTel GenAI where
   possible; journal remains the source of truth.
4. Deterministic scoring first, artifact verification, separate agent/harness
   score sheets, optional calibrated judge and human sampling.
5. Signed, hash-chained, reproducible reports with ABC checklist answers,
   cost/latency, pass@k/pass^k and confidence intervals.
6. 30 tasks in v0 (20 public, 10 private holdout), six families, oracle and
   no-op baselines.

We will **not** build a public leaderboard, a multi-vendor judge platform, or a
new task standard in this version.

## Consequences

- 1Agent scores are reported as model × harness × budget configurations, not
  model-only claims.
- External parties can run the package and verify the report without 1Agent
  internals.
- The task suite and scorer become maintenance items with versioning,
  contamination checks and retirement policy.
- Reusing Inspect/Harbor reduces scope but creates upstream dependency and
  mapping work.
- No leaderboard means less marketing surface; trust comes from reproducibility
  and third-party reruns instead.
- External audit, human calibration and GA remain separate gates; 1ASH is not
  an audit certificate.
