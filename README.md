# 1·Agent

Evidence-accountable, provider-neutral decision agent.

## Status

Version `0.53.0` is an executable V0 vertical slice:

- structured task input and deterministic cognitive-mode selection;
- static capability-based provider routing;
- canonical model messages and tool calls;
- read-only, root-confined UTF-8 file evidence tool;
- structured result-first answer contract;
- append-only JSONL run journal;
- replay and structural audit commands;
- deterministic offline provider;
- OpenAI-compatible HTTP adapter;
- provider configuration with environment-referenced credentials;
- ranked provider fallback with per-run failure memory;
- audited L2 experience candidates with review, approval, activation, degradation and rollback;
- context-budget tool-result pruning;
- explicit claim records and evidence-to-claim audit;
- durable checkpoints after complete tool batches;
- interrupted-run resume from valid terminal-free journal prefixes;
- active-experience retrieval by mode, scope and expiry with explicit unverified labeling;
- bounded, read-root-confined workspace text search;
- versioned offline evaluation suites with JSON and human-readable scorecards;
- versioned JSONL host protocol v3 with request IDs, cancellation, status and bounded concurrency;
- held-out experience verification with leakage and negative-transfer gates;
- automated baseline-vs-shadow experience evaluation that generates verification reports;
- human annotation storage and inter-rater agreement measurement;
- opt-in calibration analysis gated by reliability and sample count;
- frozen gold-set manifests with reviewer cohorts, redaction policy and batches;
- HMAC approval envelopes bound to artifact digests;
- approval-key rotation and revocation;
- signed approval-registry snapshots with rollback floors;
- hash-chained registry snapshot history;
- external trust anchors for registry history;
- reproducible build provenance for release artifacts;
- fresh-target rebuild comparison with a fail-closed reproducibility gate;
- byte-identical reproducible builds under the documented remapping profile;
- byte-identical Linux tar.gz and Windows zip archives during consecutive packages;
- fail-closed external release manifests binding source, toolchain and seven artifacts;
- optional HMAC approval for release manifests with registry key-state enforcement;
- explicit development and production release policies with anchored registry history;
- versioned release-policy files with configured signer allowlists and JSON reports;
- fail-fast provider credential readiness without printing secret values;
- opt-in live provider conformance with sanitized structured-output and usage reports;
- inert echo-tool conformance without filesystem, process or network authority;
- explicit quality, balanced, latency and cost routing objectives over hard capability filters;
- offline route plans with ranked candidates, rejection reasons and credential readiness;
- provider p95-latency and unit-cost limits as hard route constraints;
- journaled route plans with effective value sources, rejection reasons and replay;
- checkpoint-bound route-plan digests with hard rejection of changed resume policy;
- route-aware resume status and automatic recovery of the checkpoint objective;
- concise human-readable route traces without endpoint or credential disclosure;
- bounded route reasons and first-step fallback attempt counts;
- normalized OpenAI-compatible assistant content and tool-call wire formats;
- bounded JSON Schema validation before every tool execution;
- bounded and control-safe tool observations at the registry boundary;
- host-visible tool capability and schema inspection through the runtime registry;
- tool-capability snapshots bound to journal and checkpoints;
- six-case epistemic core evaluation with unknown and flip-condition gates;
- scripted reasoning fixture coverage for conflicts, flips and external correction;
- explicit contract-fixture versus model-quality evaluation suite kinds;
- model-quality suite template with non-runnable and promotion-blocking status;
- strict claim/evidence provenance invariants for epistemic answer validation;
- impact-ranked unknowns with mandatory reduction methods for high-impact gaps;
- structured, backward-compatible assumptions with impact and verification methods;
- explicit unresolved/resolved conflict records with evidence-bound resolution;
- resolution sources bound to conflict-side or explicit resolution claims;
- freshness-aware evidence with expiration and task staleness policies;
- explicit versioned answers with legacy v1 decoding and fail-closed future versions;
- unit, integration, CLI end-to-end and loopback adapter tests.

Real-model reasoning quality and full cross-platform support are not yet verified. See
[`docs/status-v0.53.md`](docs/status-v0.53.md).

Current V0 assumptions:

- Primary task: evidence-backed decision analysis.
- Authority: read-only by default.
- Initial platform: Linux validation with a platform-neutral Rust core.
- Provider strategy: deterministic test provider plus OpenAI-compatible adapter for DeepSeek, Kimi, Qwen, Ollama and similar endpoints.
- Experience evolution: L2, candidates require verification and approval before activation.

## Commands

```bash
cargo run --offline -- run --task "Compare A and B"
cargo run --offline -- run --task "Compare A and B" --route-objective balanced
cargo run --offline -- route-plan \
  --config configs/example.json \
  --task "Compare A and B" \
  --max-provider-p95-latency-ms 3000 \
  --max-unit-cost-micros-per-k 20 --json
cargo run --offline -- audit --journal .journal/run.jsonl
cargo run --offline -- replay --journal .journal/run.jsonl
cargo run --offline -- eval --suite evals/core.json
cargo run --offline -- experience list --store .experience
cargo run --offline -- resume-status --journal .journal/run.jsonl --json
cargo run --offline -- tools --json
```

Remote OpenAI-compatible provider:

```bash
AGENT_MODEL=deepseek-chat \
AGENT_BASE_URL=https://api.deepseek.com/v1 \
AGENT_API_KEY=... \
cargo run --offline -- run \
  --provider openai-compatible \
  --task "Compare A and B"
```

Multi-provider configuration:

```bash
cargo run --offline -- run \
  --config configs/example.json \
  --task "Compare A and B"
cargo run --offline -- providers --config configs/example.json --check --json
cargo run --offline -- providers \
  --config configs/example.json \
  --probe --allow-network --probe-timeout-seconds 30 --json
cargo run --offline -- providers \
  --config configs/example.json \
  --probe-tools --allow-network --probe-timeout-seconds 30 --json
```

`configs/example.json` references credentials through environment variable names. Provider
values are never written into the configuration by the runtime. `providers --check` validates
declared credentials, endpoint construction and adapter setup before a run. An omitted
`api_key_env` means no Authorization header, which supports local Ollama endpoints.
The probe is an explicit network action. It reports only status, latency, token usage and a
normalized error code; complete remote responses are not persisted or printed.
The tool probe uses only an inert echo tool and validates the complete call/observation/final
answer round trip.

The default provider is deterministic and must not be treated as a reasoning model.
For an offline read-only smoke test, use `@read-file:"<path>"` in the task objective.

## Verification

```bash
cargo fmt --check
cargo clippy --offline --all-targets -- -D warnings
cargo test --offline
./scripts/verify.sh
./scripts/verify-reproducible-package.sh
cargo run --offline -- release verify --manifest dist/1agent-0.25.0-release.json
```

A signed release check additionally requires the approval envelope, key environment variable and
key registry:

```bash
cargo run --offline -- release verify \
  --manifest dist/1agent-0.26.0-release.json \
  --approval dist/1agent-0.26.0-release.approval.json \
  --key-env RELEASE_APPROVAL_KEY \
  --registry approval-registry.json
```

A production check additionally requires `--production`, a registry signing key, one or more
ordered registry snapshots and an external trust anchor. It fails before accepting an unsigned
release or an unanchored key history.

`configs/release-policy.example.json` defines a versioned production policy and signer
allowlists. Use `--policy` with `--json` to produce a machine-readable verification report.

The package reproducibility gate performs four complete builds and compares both archives
and the manifest byte-for-byte. It also extracts both archives, verifies their checksums and
compares their contents. It validates local repeatability; cross-machine hermeticity is not yet
claimed.

The OpenAI-compatible loopback test requires a managed-sandbox exception:

```bash
AGENT_RUN_LOOPBACK_TESTS=1 ./scripts/verify.sh
```
