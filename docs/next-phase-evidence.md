# Next-Phase Evidence Plan

Date: 2026-11-02

Status: proposed; no live-provider or Windows-host claim has been verified yet

## Purpose

Stop expanding autonomy until the current runtime has evidence in the environments where it is
expected to make decisions. This plan converts the remaining unknowns into experiments with
explicit pass, fail and reversal conditions.

## Questions

1. Does the answer contract reduce unsupported certainty across providers?
2. Are decision completeness, grounded correctness and task usefulness distinct enough to gate
   separately?
3. How stable are results under repeated runs and parameter changes?
4. When do retries and fallback help, and when do they hide deterministic provider failures?
5. What latency and cost are required to achieve an acceptable quality level?
6. Does the same binary and journal behavior hold on a real Windows host?
7. Can another machine reconstruct the released artifacts from source?

## Evaluation corpus

Use versioned suites with disjoint `development`, `tuning` and `held-out` groups.

| Task family | Minimum coverage | Primary gate |
|---|---:|---|
| Evidence reading and source attribution | 12 cases | evidence-to-claim provenance |
| Conflicting sources and recency | 12 cases | unresolved conflict and flip binding |
| Decision tradeoffs and constraints | 12 cases | decision completeness and recommendation quality |
| Insufficient information and escalation | 12 cases | honest uncertainty and minimal question |
| Tool calls and malformed observations | 12 cases | structured call/observation/stop reason |
| Interruption, resume and retry | 12 cases | replay equivalence and budget compliance |
| Prompt-injection and unsafe controls | 12 cases | safety and permission boundaries |

Case design requirements:

- independently reviewed expected criteria;
- no tuning cases reused as held-out evidence;
- source timestamps for freshness-sensitive cases;
- at least two reviewer labels for subjective recommendation cases;
- explicit rubric for unsupported certainty and high-impact unknowns;
- fixed provider/model/version metadata for every run.

## Provisional acceptance gates

These are starting gates, not calibrated product claims. They must be revised only with recorded
evidence.

| Stage | Corpus | Repetitions | Gate |
|---|---:|---:|---|
| Adapter smoke | 6 cases | 1 | all requests complete or fail with normalized errors |
| Provider pilot | 84 cases | 3 | hard-gate failures `0`; report confidence intervals |
| Routing pilot | 84 cases | 5 | fallback improves completion without hidden data leakage |
| Production candidate | 300+ cases | 5 | reviewed thresholds, calibrated uncertainty and cost envelope |

Quality reports must include:

- pass rate and pass^k;
- decision-completeness rate and floor source;
- hard-gate failure count;
- unsupported-certainty rate;
- evidence provenance failures;
- p50/p95 latency;
- token and monetary cost;
- provider retry and fallback outcomes;
- reviewer agreement for subjective labels.

## Provider matrix

| Provider route | Required evidence | Current state |
|---|---|---|
| DeepSeek via OpenAI-compatible adapter | live conformance, structured output, cost/latency, held-out quality | not verified |
| Kimi via compatible adapter | same | not verified |
| Qwen via compatible adapter | same | not verified |
| Local Ollama | live conformance, model identity, tool behavior, resource use | not verified |
| Generic compatible endpoint | contract probe and provider-specific override review | adapter implemented, live quality unverified |

Each provider run must record:

- endpoint class without secrets;
- provider ID and model identifier;
- adapter version;
- temperature and other decoding parameters;
- timeout, retry count and fallback path;
- input/output usage and cost;
- sanitized error code;
- report content digest and optional approval proof.

## Cross-platform gates

### Windows

- run unit tests on a real Windows host;
- execute run, audit, replay, eval, eval-inspect, experience and stdio host smoke tests;
- verify path confinement, journal writes, cancellation and process termination;
- compare JSON contracts with Linux.

### Reproducibility

- rebuild approved source on a second physical or virtual host;
- use the documented toolchain profile;
- compare binaries, archives and release manifests byte-for-byte;
- verify provenance and signature/key-registry procedures;
- state clearly if cross-machine hermeticity remains unproven.

## Decision and reversal conditions

### Continue

Continue to production-candidate evaluation when:

- all hard gates pass on the held-out corpus;
- provider-specific failures are normalized and explainable;
- decision completeness is measured independently from answer correctness;
- cost and tail latency fit the declared product envelope;
- reports are inspectable and integrity-bound.

### Revisit architecture

Revisit routing or answer contracts when:

- quality differs materially by provider but routing cannot express the capability difference;
- the structured answer improves completeness while reducing useful task success;
- retries or fallbacks hide deterministic quality failures;
- reviewers cannot agree on decision-quality labels;
- context, tool, or latency budgets dominate outcomes.

### Stop expansion

Stop adding autonomy when:

- hard-gate failures remain on held-out cases;
- unsupported certainty increases with more memory/experience;
- self-evolution has no measured held-out benefit;
- permissions, secrets or external content cannot be audited;
- no target host can reproduce the release.

## Work order

1. Approve the corpus, rubric and reviewer process.
2. Configure one real provider and execute adapter smoke tests.
3. Run the provider pilot and preserve raw reports.
4. Repeat with a second provider and local Ollama.
5. Evaluate route policy and fallback tradeoffs.
6. Execute the Windows and cross-host gates.
7. Decide whether to keep, revise or stop the current V0 architecture.
8. Only then evaluate asymmetric report signatures or higher autonomy.
