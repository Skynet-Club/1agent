# Changelog

## 0.44.0 - 2026-11-02

- Added a scripted reasoning fixture provider for evaluation-framework validation.
- Covered contradictory sources with explicit conflicting claims.
- Covered conditional decisions with flip conditions.
- Covered self-correction only after a real file observation.
- Clearly separated fixture behavior from real-model quality evidence.

## 0.43.0 - 2026-11-02

- Added minimum unknown and flip-condition expectations to evaluation cases.
- Added unknown/flip-condition counts to JSON scorecards.
- Expanded the core suite from four to six cases.
- Added missing-file evidence failure coverage.
- Added zero-width hidden-control rejection distinct from bidirectional controls.
- Upgraded the suite to `core-v2` with explicit suite version `2`.

## 0.42.0 - 2026-11-02

- Added compact runtime tool-capability snapshots with schema digest and tool names.
- Journaled the snapshot before execution.
- Bound checkpoints to the snapshot digest.
- Rejected resume when tool schemas or capability metadata change.
- Preserved legacy checkpoints without a snapshot.
- Exposed the tool snapshot digest through resume status.
- Added replay and changed-registry resume coverage.

## 0.41.0 - 2026-11-02

- Added `tools --json` for host-visible capability inspection.
- Reports tool names, descriptions, schemas, risk, write requirement, timeout and status.
- Uses the same runtime registry/spec source as execution.
- Added implemented/degraded/unsupported capability status taxonomy.
- Added end-to-end coverage for tool inventory and path redaction.

## 0.40.0 - 2026-11-02

- Added central observation summary/content bounds.
- Preserved truncation metadata and a general `truncated` marker.
- Rejected hidden/bidirectional controls across the final observation boundary.
- Preserved structured observation status and error codes.
- Added adversarial oversized and unsafe observation coverage.

## 0.39.0 - 2026-11-02

- Added a bounded JSON Schema subset validator at the central tool boundary.
- Validates object shape, required fields, additional properties, base types, string/integer
  bounds, array items and enums.
- Rejects invalid arguments before tool-specific execution.
- Kept path containment, side effects, permissions and concurrency checks inside each tool.
- Added nested-schema and unexpected-field coverage.

## 0.38.0 - 2026-11-02

- Added OpenAI-compatible assistant content normalization for strings and text-part arrays.
- Rejected unsupported or malformed content parts.
- Assigned deterministic IDs to missing tool-call IDs.
- Rejected duplicate, empty or malformed tool names and duplicate call IDs.
- Kept provider wire types isolated from the domain/tool contracts.
- Added adapter normalization tests.

## 0.37.0 - 2026-11-02

- Bounded human-readable route reasons to 240 characters.
- Added first-model-step fallback-attempt count to route summaries.
- Moved route-summary formatting into a testable library function.
- Added bounded-output and fallback-count coverage.
- Left complete, untruncated reasons in journal/replay JSON.

## 0.36.0 - 2026-11-02

- Added concise route summaries to non-JSON run and resume output.
- Displayed mode, objective, candidate/rejection counts, first selected route and its reason.
- Displayed policy-source attribution.
- Kept full plans in JSON journal/replay output.
- Ensured traces omit endpoints, credentials and prompt content.
- Added route-trace summary coverage.

## 0.35.0 - 2026-11-02

- Added `resume-status` for inspecting checkpoint route metadata without resuming.
- Added explicit route-aware versus legacy checkpoint status.
- Resume now reuses the checkpoint objective when no CLI override is provided.
- Explicit conflicting objective overrides fail before provider execution.
- Preserved stored policy-source attribution during resume.
- Added CLI coverage for status, automatic objective recovery and conflicting override rejection.

## 0.34.0 - 2026-11-02

- Bound checkpoints to the canonical route plan and policy-source digest.
- Persisted effective route policy sources in each checkpoint.
- Resume recomputes the plan and rejects changed objective, budgets, provider manifests or policy
  sources.
- Preserved legacy checkpoints without route digests.
- Added interruption/resume policy-change rejection coverage.

## 0.33.0 - 2026-11-02

- Added `RoutePlanned` journal events with ranked and rejected candidates.
- Journaled objective, latency-budget and unit-cost budget value sources.
- Extended replay state and CLI replay output to recover route plans and policy sources.
- Added audit rejection when an executed route is absent from its preceding route plan.
- Preserved old journals that contain no route-plan event.
- Added runtime replay and negative route-selection audit coverage.

## 0.32.0 - 2026-11-02

- Added provider p95 latency and unit-cost limits to run budgets.
- Added matching hard-filter rejection reasons to route plans and runtime routing.
- Added CLI overrides for both limits on `run` and `route-plan`.
- Journaled effective limits through the initial task request.
- Added end-to-end coverage proving over-budget candidates are excluded with both reasons.

## 0.31.0 - 2026-11-02

- Added `route-plan` for offline provider selection inspection.
- Exposed ranked candidates and each hard-filter rejection reason.
- Added per-provider credential and endpoint readiness to route-plan output.
- Reused the runtime Router implementation, preventing planning/execution drift.
- Added end-to-end coverage proving remote and tool-capability rejections are reported without
  network calls.

## 0.30.0 - 2026-11-02

- Added quality, balanced, latency and cost route objectives.
- Kept mode, risk, privacy, tool and structured-output checks as hard filters.
- Added deterministic objective-specific ordering and tie-breaking.
- Added `--route-objective` to run and resume commands.
- Recorded the selected objective in route-selection reasons.
- Added coverage proving objective changes ranking while hard filters remain enforced.

## 0.29.0 - 2026-11-02

- Added an inert echo tool for provider tool-calling conformance.
- Added `providers --probe-tools --allow-network`.
- Validated exactly one tool call, non-empty call ID, allowed tool name and JSON arguments.
- Executed the echo observation without read, write, process or network authority.
- Required a second model call to produce a valid final answer after the observation.
- Added sanitized tool-call/observation/final-answer capability results and usage accounting.

## 0.28.0 - 2026-11-02

- Added explicit opt-in live conformance probing for OpenAI-compatible providers.
- Probes validate authentication/network handling, final structured output and usage extraction.
- Reports are sanitized to status, latency, token counts and normalized error codes.
- Network probing requires `--allow-network`.
- Added deterministic contract-validation coverage and retained offline readiness as a separate
  state.

## 0.27.0 - 2026-11-02

- Added `providers --check` readiness validation.
- Declared `api_key_env` values now fail at provider construction when missing.
- Omitted `api_key_env` remains valid for unauthenticated local endpoints such as Ollama.
- Rejected empty credential variable names and credentials on deterministic providers.
- Added child-process coverage proving readiness succeeds with a credential and never prints its
  value.

## 0.26.0 - 2026-11-02

- Added versioned release-policy files.
- Bound release mode, minimum registry version, manifest signer IDs and registry signer IDs.
- Added signer allowlist enforcement for release approvals and registry snapshot chains.
- Added machine-readable release verification reports.
- Added `configs/release-policy.example.json`.
- Kept ad hoc production flags compatible while allowing policy files to define the mode and
  minimum registry version.

## 0.25.0 - 2026-11-02

- Added explicit development and production release verification policies.
- Production requires release approval, registry key state, a signing key environment variable,
  an ordered registry snapshot chain and an external trust anchor.
- Added registry chain options to `release verify`.
- Rejected production-only options outside production mode.
- Re-checked the manifest and all artifacts after approval and chain verification.
- Verified production policy with a real CLI snapshot-chain and retired-key scenario.

## 0.24.0 - 2026-11-02

- Added `release_manifest` to the approval artifact taxonomy.
- Added expected-kind checking for approval envelopes.
- Added optional signed verification to `release verify`.
- Enforced registry key state for signed release verification.
- Verified retired keys can validate historical releases but cannot sign new releases.
- Verified revoked keys and mismatched artifact kinds fail closed.

## 0.23.0 - 2026-11-02

- Added strict, versioned external release manifests.
- Bound package version, clean Git revision, Rust toolchain and seven release artifacts.
- Required complete `SHA256SUMS` coverage for every packaged file.
- Rejected dirty releases, missing or duplicate roles, path traversal and digest drift.
- Added `release manifest` and `release verify` CLI commands.
- Integrated manifest generation into packaging and manifest equality into the release gate.
- Added archive extraction, checksum, provenance and cross-format content verification.

## 0.22.0 - 2026-11-02

- Added deterministic provenance timestamps through `SOURCE_DATE_EPOCH`.
- Recreated package output and archives before every package run.
- Generated checksums before normalizing all packaged file timestamps.
- Added sorted, ownership-neutral tar and deterministic gzip metadata.
- Added sorted zip output with normalized timestamps and no directory entries.
- Added `scripts/verify-reproducible-package.sh`.
- Verified consecutive package runs produce byte-identical Linux and Windows archives.

## 0.21.0 - 2026-11-02

- Isolated build-path reproducibility differences.
- Fixed SOURCE_DATE_EPOCH, incremental state, symbol stripping and source/target path remapping.
- Added `scripts/reproducible-build.sh`.
- Produced byte-identical Linux builds: SHA-256 `40b6b6...c484`, size `7,744,216`.
- Integrated double-build reproducibility verification into packaging.

## 0.20.0 - 2026-11-02

- Added `provenance rebuild`.
- Added fresh target-directory release builds.
- Added byte-for-byte hash and size comparison.
- Added fail-closed reproducer promotion.
- Recorded the current Linux rebuild as non-reproducible.

## 0.19.0 - 2026-11-02

- Added build provenance with commit, dirty state and tool versions.
- Added target triple, artifact size and SHA-256.
- Added `provenance create/verify`.
- Added optional signed provenance approval.
- Package output now includes Linux and Windows provenance.
- Added artifact-change rejection tests.

## 0.18.0 - 2026-11-02

- Added external trust-anchor manifests.
- Added `approval anchor`.
- Added `--anchor` verification for snapshot chains.
- Bound anchor to first snapshot digest, operator and authorized key ids.
- Required calibration snapshot chains to start at the trusted anchor.
- Added alternate-history and unauthorized-key tests.

## 0.17.0 - 2026-11-02

- Added previous-snapshot digest links and snapshot self-digests.
- Added `approval verify-chain`.
- Added contiguous chain verification.
- Rejected forks, out-of-order history and repeated versions.
- Calibration now requires a valid snapshot chain ending at current registry.
- Added fork and ordering tests.

## 0.16.0 - 2026-11-02

- Added signed approval-registry snapshots.
- Bound registry SHA-256, monotonic version, approver and admin key id.
- Added `approval snapshot/verify-snapshot`.
- Added trusted minimum registry version checks.
- Rejected registry rollback and post-snapshot mutation.
- Required snapshot validation in calibration when a registry is used.

## 0.15.0 - 2026-11-02

- Added approval key registry with active/retired/revoked states.
- Added `approval key-add`, `key-retire` and `key-revoke`.
- Retired keys verify historical approvals but cannot sign new ones.
- Revoked keys cannot verify approvals.
- Calibration requires registry-based approval verification.
- Added key-rotation and revocation tests.

## 0.14.0 - 2026-11-02

- Added detached HMAC-SHA256 approval envelopes.
- Bound approvals to artifact SHA-256, approver, purpose, timestamp and key id.
- Added `approval sign/verify`.
- Required signed gold-set approval for calibration.
- Added artifact-change and MAC-tampering tests.
- Kept unsigned artifacts development-only.

## 0.13.0 - 2026-11-02

- Added frozen gold-set manifest creation and verification.
- Added suite SHA binding, reviewer cohort, owner, redaction policy and batch assignments.
- Added manifest self-digest and drift detection.
- Required gold-set validation for calibration CLI runs.
- Added prediction rejection outside the frozen case set.
- Added gold-set manifest digest tests.

## 0.12.0 - 2026-11-02

- Added `annotate calibrate` for opt-in reliability-gated calibration analysis.
- Added ECE, Brier score, reliability bins and selective-risk points.
- Added kappa and sample-count hard gates.
- Added refusal when labels or predictions are insufficient.
- Added calibration tests for allowed and refused evidence states.

## 0.11.0 - 2026-11-02

- Added `annotate add/list/agreement`.
- Added supported/unsupported/needs-evidence labels with evidence sources and notes.
- Added duplicate case/annotator protection and hidden-control rejection.
- Added pair observed/expected agreement and kappa measurement.
- Added insufficient-data and disagreement tests.

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
