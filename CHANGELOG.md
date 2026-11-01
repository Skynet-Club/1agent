# Changelog

## 0.10.0 - 2026-11-02

- Added `experience evaluate` for automated baseline-vs-shadow held-out evaluation.
- Added EvalSuite version metadata and per-case experience scopes.
- Added measured success, unsupported-certainty and duration-delta metrics.
- Added SHA-256 and case-id binding generation from the actual suite file.
- Added shadow candidate injection without activating unverified experience.
- Added positive learned-transfer and negative false-certainty tests.
- Added human-authored evaluation rubrics for conclusion terms, sources and claim statuses.
- Added rubric pass/check totals to evaluation scorecards.

## 0.9.0 - 2026-11-01

- Added `verified` experience state between review and approval.
- Added held-out verification reports with separate training/evaluation groups.
- Added gates for success improvement, unsupported-certainty regression and cost growth.
- Added `experience verify` CLI.
- Added negative tests for data leakage and increased false certainty.
- Added request-id persistence to run journals and host status journal paths.
- Added bounded journal history discovery with client request id recovery.
- Bound experience verification reports to suite version, unique case ids and SHA-256.

## 0.8.0 - 2026-11-01

- Added versioned JSONL stdio host protocol v3.
- Added stable request IDs and structured success/error responses.
- Added request-size, JSON and protocol-version validation.
- Added `serve` CLI command with configured providers.
- Added a real subprocess integration test and auditable host response.
- Added a documented host request example.
- Added `cancel` control messages keyed by request id.
- Added immediate `queue_full` and `duplicate_request` responses.
- Added structured cancellation responses and real slow-provider tests.
- Added secret-safe `providers` configuration inspection.
- Added host `status` queries with bounded recent lifecycle history.
- Added bounded journal-tail history discovery without loading observation bodies.
- Persisted host request ids in authoritative run journals.
- Added journal paths to terminal host status entries.

## 0.7.0 - 2026-11-01

- Added versioned offline evaluation suites.
- Added isolated workspace materialization for every case.
- Added expected stop reason, answer status, evidence count, claim count and audit checks.
- Added hard-gate failure accounting.
- Added machine-readable JSON reports and a human-readable CLI scorecard.
- Added `eval` CLI command and `evals/core.json`.
- Added checkpoint and tool-outcome expectations.
- Added provider, model, elapsed time and token usage to case reports.
- Verified the core suite with 4/4 cases and zero hard-gate failures.
- Produced a Windows GNU x86-64 PE release artifact; runtime execution remains unverified.

## 0.6.0 - 2026-11-01

- Added bounded workspace-wide `search_files` read-only tool.
- Added path containment, symlink, depth, file-size, file-count and result limits.
- Added generated-directory skipping.
- Added file, line, column and bounded excerpt output.
- Added hidden-control rejection for searched files.
- Added search coverage for generated-directory exclusion and path-escape rejection.

## 0.5.0 - 2026-11-01

- Added active-experience retrieval by cognitive mode, scope and expiry.
- Added context-budget-limited experience injection.
- Marked injected experience as approved but unverified policy guidance.
- Journaled candidate id, version and selection reason for every injection.
- Added CLI `--experience-dir` and `--experience-scope`.
- Added negative coverage proving draft and out-of-scope candidates are not injected.

## 0.4.0 - 2026-11-01

- Added checkpoints after every complete tool batch.
- Added resumable-prefix validation for sequence, run identity, tool pairing and terminal state.
- Added `resume` CLI and runtime API.
- Preserved canonical messages, mode, tool count, elapsed budget and provider suppression state.
- Linked each resumed journal to its original run and checkpoint sequence.
- Added runtime and real CLI interruption-equivalence tests.
- Rejected resume from completed runs.

## 0.3.0 - 2026-11-01

- Added multi-provider JSON configuration with environment-referenced API keys.
- Added ranked provider fallback for retryable failures and per-run failed-route suppression.
- Added deterministic context-budget pruning while preserving user text and recent messages.
- Added explicit claim records and evidence-to-claim reference validation.
- Upgraded journal audit to validate answers, claim references and stop-reason consistency.
- Added shared detection for bidi, zero-width, tag and unsafe control characters.
- Added config-file CLI integration coverage.
- Verified Windows GNU cross-target compilation after the new modules.

## 0.2.0 - 2026-11-01

- Added L2 experience candidates for memory, skill and route-policy lessons.
- Bound candidates to audited run journals and existing evidence event sequences.
- Added review, approval, rejection, activation, degradation and rollback transitions.
- Added invisible Unicode and empty-text rejection for experience writes.
- Added CLI commands for the complete candidate lifecycle.
- Added unit coverage and a real CLI end-to-end experience lifecycle test.
- Verified Windows GNU cross-target compilation with `cargo check`.

## 0.1.0 - 2026-11-01

- Created the Rust/Tokio agent runtime.
- Added structured task, answer, model, tool and journal contracts.
- Added deterministic provider and OpenAI-compatible adapter.
- Added capability-based static provider routing.
- Added read-only, root-confined file evidence tool.
- Added JSONL replay and audit commands.
- Added unit, CLI end-to-end and loopback provider tests.
- Added architecture, ADR and verification documentation.
