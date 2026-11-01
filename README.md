# 1·Agent

Evidence-accountable, provider-neutral decision agent.

## Status

Version `0.23.0` is an executable V0 vertical slice:

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
- unit, integration, CLI end-to-end and loopback adapter tests.

Real-model reasoning quality and full cross-platform support are not yet verified. See
[`docs/status-v0.23.md`](docs/status-v0.23.md).

Current V0 assumptions:

- Primary task: evidence-backed decision analysis.
- Authority: read-only by default.
- Initial platform: Linux validation with a platform-neutral Rust core.
- Provider strategy: deterministic test provider plus OpenAI-compatible adapter for DeepSeek, Kimi, Qwen, Ollama and similar endpoints.
- Experience evolution: L2, candidates require verification and approval before activation.

## Commands

```bash
cargo run --offline -- run --task "Compare A and B"
cargo run --offline -- audit --journal .journal/run.jsonl
cargo run --offline -- replay --journal .journal/run.jsonl
cargo run --offline -- eval --suite evals/core.json
cargo run --offline -- experience list --store .experience
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
```

`configs/example.json` references credentials through environment variable names. Provider
values are never written into the configuration by the runtime.

The default provider is deterministic and must not be treated as a reasoning model.
For an offline read-only smoke test, use `@read-file:"<path>"` in the task objective.

## Verification

```bash
cargo fmt --check
cargo clippy --offline --all-targets -- -D warnings
cargo test --offline
./scripts/verify.sh
./scripts/verify-reproducible-package.sh
cargo run --offline -- release verify --manifest dist/1agent-0.23.0-release.json
```

The package reproducibility gate performs four complete builds and compares both archives
and the manifest byte-for-byte. It also extracts both archives, verifies their checksums and
compares their contents. It validates local repeatability; cross-machine hermeticity is not yet
claimed.

The OpenAI-compatible loopback test requires a managed-sandbox exception:

```bash
AGENT_RUN_LOOPBACK_TESTS=1 ./scripts/verify.sh
```
