# Model Routing

Date: 2026-11-02

Status: static capability routing implemented; live quality data pending

## Goal

Keep model selection outside the domain reasoner. A provider/model is eligible only when it
satisfies hard task requirements; eligible routes are then ordered by an explicit objective.

## Provider manifest

Every provider candidate declares:

```text
provider_id
model
privacy
supported_modes
supports_tools
supports_structured_output
max_risk
quality_score
input_cost_micros_per_k
output_cost_micros_per_k
p95_latency_ms
```

These values are configuration inputs, not learned facts. Until measured, a quality score is a
prior and must not be described as benchmark performance.

## Hard filters

A route is rejected when:

- the cognitive mode is unsupported;
- task risk exceeds the provider maximum;
- tools are required but unsupported;
- structured output is required but unsupported;
- remote privacy is disallowed;
- configured p95 latency is exceeded;
- configured unit cost is exceeded.

Hard filters are identical for every objective. Ranking cannot override a safety, privacy or
capability rejection.

## Objectives

### Quality

Order by:

1. descending `quality_score`;
2. ascending combined unit cost;
3. ascending p95 latency;
4. provider ID and model ID for ties.

### Latency

Order by:

1. ascending p95 latency;
2. descending quality;
3. ascending cost;
4. stable identity.

### Cost

Order by:

1. ascending combined input/output unit cost;
2. descending quality;
3. ascending latency;
4. stable identity.

### Balanced

Compute independent ordinal ranks for quality, cost and latency, then minimize the sum of those
ranks. This is an ordinal compromise, not a calibrated score or probability.

Balanced ranking does not imply that one quality point equals one millisecond or one cost unit.
User-configurable weights remain future work.

## Route plan

The runtime records:

- route request;
- ordered eligible decisions;
- rejection reasons for every ineligible provider;
- effective objective, latency budget and cost-budget sources.

Rejected routes remain visible for audit. Provider endpoints and credentials are not included in
the plan report.

## Fallback

Fallback occurs only for retryable provider failures such as network, timeout, rate limit or
provider unavailability.

Fallback does not occur for:

- authentication or configuration failure;
- cancellation;
- invalid structured output;
- an exhausted provider-attempt budget;
- an exhausted run budget.

Each failed attempt is journaled with sanitized provider/model identity, stable error code and
retryability. Retry scheduling records the next target, deterministic bounded delay and timing
outcome.

## Retry bounds

Default maximum provider attempts: `4`.

Default initial backoff: `50 ms`.

Backoff grows deterministically with a hard cap of `1 second`. The runtime checks remaining run
time before waiting or selecting another provider.

## Resume binding

Checkpoints bind:

- canonical route plan digest;
- effective route-policy sources;
- unavailable routes;
- provider attempts used;
- tool-capability digest.

Resume rejects changed hard routing policy or capability state.

## Routing decision conditions

Revisit the router when:

- provider quality differences cannot be represented by one `quality_score`;
- task-specific evidence shows mode-specific ranking;
- cost or latency thresholds vary materially by task family;
- retries produce different success distributions by provider;
- fallback hides deterministic contract failures;
- measured p95 differs from configured latency;
- privacy or residency requirements need region-level controls.

## Not implemented

- online learned routing;
- token-aware per-run monetary budget;
- calibrated provider quality estimates;
- per-task-family weights;
- live-provider quality benchmarks;
- macOS/Windows runtime latency calibration.
