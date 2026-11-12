# Architecture Decision Records

One line per decision. New ADRs are added as numbered Markdown files in this directory.

| # | Decision | File |
|---|---|---|
| 0001 | Rust and Tokio Runtime | [0001-rust-runtime.md](0001-rust-runtime.md) |
| 0002 | JSONL Run Journal | [0002-jsonl-journal.md](0002-jsonl-journal.md) |
| 0003 | OpenAI-compatible Provider Adapter | [0003-openai-compatible-provider.md](0003-openai-compatible-provider.md) |
| 0004 | Evaluation Report Trust Boundary | [0004-evaluation-report-trust.md](0004-evaluation-report-trust.md) |
| 0005 | Terminal UI as a Presentation Adapter | [0005-terminal-ui-presentation-adapter.md](0005-terminal-ui-presentation-adapter.md) |
| 0006 | Raw Terminal Presentation | [0006-raw-terminal-presentation.md](0006-raw-terminal-presentation.md) |
| 0007 | Ratatui Interaction Layer | [0007-ratatui-interaction-layer.md](0007-ratatui-interaction-layer.md) |
| 0008 | Host-Aware TUI Command Scopes and Session Events | [0008-tui-command-scopes-and-session-events.md](0008-tui-command-scopes-and-session-events.md) |
| 0009 | Explicit TUI Command Modes and Journal-Derived Evidence Views | [0009-explicit-command-modes-and-evidence-views.md](0009-explicit-command-modes-and-evidence-views.md) |
| 0010 | Session-Bound Master Plan Approval | [0010-session-bound-master-plan-approval.md](0010-session-bound-master-plan-approval.md) |
| 0011 | Session-to-Master Dispatch and Explicit Route Selection | [0011-session-to-master-dispatch.md](0011-session-to-master-dispatch.md) |
| 0012 | Master Supervision and Human Response | [0012-master-supervision-and-human-response.md](0012-master-supervision-and-human-response.md) |
| 0013 | Explicit Artifact Verification and Master Views | [0013-explicit-artifact-verification-views.md](0013-explicit-artifact-verification-views.md) |
| 0014 | Token Headroom, Session Workspace and Local Presets | [0014-token-headroom-session-workspace-presets.md](0014-token-headroom-session-workspace-presets.md) |
| 0015 | Provider Usage Ledger and Threshold Compaction | [0015-provider-usage-ledger-and-threshold-compaction.md](0015-provider-usage-ledger-and-threshold-compaction.md) |
| 0016 | Unified Capability Probe Evidence | [0016-unified-capability-probe-evidence.md](0016-unified-capability-probe-evidence.md) |
| 0017 | Goal, Key Result and Task List Snapshots | [0017-goal-key-result-task-list.md](0017-goal-key-result-task-list.md) |
| 0018 | Deterministic Global Scheduling Across Master Conversations | [0018-deterministic-global-scheduler.md](0018-deterministic-global-scheduler.md) |
| 0019 | 回答之前先判断请求是否为手段 | [0019-requirement-regression.md](0019-requirement-regression.md) |
| 0020 | 用 journal 承载需求重构的提议与决定 | [0020-reframe-journal-events.md](0020-reframe-journal-events.md) |
| 0021 | Backup Export and Hard-Forget Boundary | [0021-backup-hard-forget-boundary.md](0021-backup-hard-forget-boundary.md) |
| 0022 | Surface HTTP Principal and Context Scoping | [0022-surface-server-principal.md](0022-surface-server-principal.md) |
| 0023 | Scope KEK Ledger and Rotation | [0023-scope-kek-ledger-and-rotation.md](0023-scope-kek-ledger-and-rotation.md) |
| 0024 | Runtime Home and Export Boundary | [0024-runtime-home-and-export-boundary.md](0024-runtime-home-and-export-boundary.md) |
| 0025 | Local Key Escrow | [0025-local-key-escrow.md](0025-local-key-escrow.md) |
| 0026 | Cross-Process Journal and Snapshot Locks | [0026-task-graph-cross-process-write-lock.md](0026-task-graph-cross-process-write-lock.md) |
| 0027 | External Alert Webhook | [0027-external-alert-webhook.md](0027-external-alert-webhook.md) |
| 0028 | Standalone TUI Turn Completion | [0028-standalone-tui-turn-completion.md](0028-standalone-tui-turn-completion.md) |
| 0029 | Command Drawer, Focus and Arrow-Key Semantics | [0029-command-drawer-focus-and-keys.md](0029-command-drawer-focus-and-keys.md) |
| 0030 | Guarded Quit, Direct Answers and Run Metrics | [0030-tui-quit-direct-answer-and-metrics.md](0030-tui-quit-direct-answer-and-metrics.md) |
| 0031 | Execution and File Writes by Default | [0031-execution-by-default.md](0031-execution-by-default.md) |
| 0032 | Local PDF Text Extraction via `pdftotext` | [0032-local-pdf-text-extraction.md](0032-local-pdf-text-extraction.md) |
| 0033 | Skill Bundles and Skill-Bound Routines | [0033-skill-bundles-and-routines.md](0033-skill-bundles-and-routines.md) |
| 0034 | Token-Based Context Budget for 1M-Token Sessions | [0034-token-context-budget.md](0034-token-context-budget.md) |
| 0035 | User Model 2.0 — Autonomy, Voice, and Memory Consent | [0035-user-model-consent.md](0035-user-model-consent.md) |
| 0036 | Platform Capability Registry | [0036-platform-capability-registry.md](0036-platform-capability-registry.md) |
| 0037 | Independent Verification and Hack-Verifiable Acceptance | [0037-independent-verification.md](0037-independent-verification.md) |
| 0038 | Trust & Repair — Side-Effect Repair Plans and Away Digest | [0038-trust-and-repair.md](0038-trust-and-repair.md) |
| 0039 | Value Report v0 — Outcomes and Cost, Not Invented Time Savings | [0039-value-report-v0.md](0039-value-report-v0.md) |
| 0040 | Partner Voice Modes — Operator, Advisor, Companion | [0040-partner-voice-modes.md](0040-partner-voice-modes.md) |
| 0041 | Agent Inbox Store — Durable Notify/Question/Review Queue | [0041-agent-inbox-store.md](0041-agent-inbox-store.md) |
| 0042 | Agent Inbox CLI and Source Projections | [0042-agent-inbox-cli-and-projections.md](0042-agent-inbox-cli-and-projections.md) |
| 0043 | Inbox Sync — Merge Projections with Dedupe and Dry-Run | [0043-inbox-sync.md](0043-inbox-sync.md) |
| 0044 | Autonomy Dial Policy | [0044-autonomy-dial.md](0044-autonomy-dial.md) |
| 0045 | Onboarding v0 — Non-Interactive Profile Setup | [0045-onboarding-v0.md](0045-onboarding-v0.md) |
| 0046 | TUI `/inbox` View and Inline Actions | [0046-tui-inbox-command.md](0046-tui-inbox-command.md) |
| 0047 | Agent Card v0 | [0047-agent-card-v0.md](0047-agent-card-v0.md) |
| 0048 | Skill Scan Command and Injection Markers | [0048-skill-scan-command.md](0048-skill-scan-command.md) |
| 0049 | Tool Search v0 | [0049-tool-search-v0.md](0049-tool-search-v0.md) |
| 0050 | Wire Tool Search into Request Construction | [0050-tool-search-request-wiring.md](0050-tool-search-request-wiring.md) |
| 0051 | Serve the Agent Card over the HTTP Surface | [0051-serve-agent-card.md](0051-serve-agent-card.md) |
| 0052 | Skill Bundle Verify Command | [0052-skill-bundle-verify.md](0052-skill-bundle-verify.md) |
| 0053 | Computer-Use Provenance and Branch-Steering Policy | [0053-cua-provenance-policy.md](0053-cua-provenance-policy.md) |
| 0054 | Cascade Containment Metrics | [0054-cascade-containment-metrics.md](0054-cascade-containment-metrics.md) |
| 0055 | Budget Policy and Cost per Verified Outcome | [0055-budget-policy.md](0055-budget-policy.md) |
| 0056 | OpenTelemetry GenAI-Style Span Projection | [0056-otel-genai-spans.md](0056-otel-genai-spans.md) |
| 0057 | Chat Adapter Contract | [0057-chat-adapter-contract.md](0057-chat-adapter-contract.md) |
| 0058 | Team Role and Scope Policy | [0058-team-role-policy.md](0058-team-role-policy.md) |
| 0059 | Agent Card Signing | [0059-agent-card-signing.md](0059-agent-card-signing.md) |
| 0060 | Durable Idempotency Records | [0060-durable-idempotency-records.md](0060-durable-idempotency-records.md) |
| 0061 | Out-of-Band Daemon Watchdog | [0061-out-of-band-daemon-watchdog.md](0061-out-of-band-daemon-watchdog.md) |
| 0062 | Autonomy Quiet Hours | [0062-autonomy-quiet-hours.md](0062-autonomy-quiet-hours.md) |
| 0063 | False-Alarm Budget | [0063-false-alarm-budget.md](0063-false-alarm-budget.md) |
| 0064 | Escalation Ladder | [0064-escalation-ladder.md](0064-escalation-ladder.md) |
| 0065 | Organization Audit Index | [0065-organization-audit-index.md](0065-organization-audit-index.md) |
| 0066 | MCP OAuth Claims Policy | [0066-mcp-oauth-claims-policy.md](0066-mcp-oauth-claims-policy.md) |
| 0067 | Agent Card Trust Store | [0067-agent-card-trust-store.md](0067-agent-card-trust-store.md) |
| 0068 | Team Handoff | [0068-team-handoff.md](0068-team-handoff.md) |
| 0069 | Handoff Inbox Projection | [0069-handoff-inbox-projection.md](0069-handoff-inbox-projection.md) |
| 0070 | MCP Streamable HTTP Transport | [0070-mcp-streamable-http-transport.md](0070-mcp-streamable-http-transport.md) |
| 0071 | Cost per Verified Outcome | [0071-cost-per-verified-outcome.md](0071-cost-per-verified-outcome.md) |
| 0072 | Onboarding Context Import | [0072-onboarding-context-import.md](0072-onboarding-context-import.md) |
| 0073 | Inbox Handoff Acceptance | [0073-inbox-handoff-acceptance.md](0073-inbox-handoff-acceptance.md) |
| 0074 | Organization Audit Seals | [0074-organization-audit-seals.md](0074-organization-audit-seals.md) |
| 0075 | Digest Attention Summary | [0075-digest-attention-summary.md](0075-digest-attention-summary.md) |
| 0076 | Trust Disclosure Manifest | [0076-trust-disclosure-manifest.md](0076-trust-disclosure-manifest.md) |
| 0077 | Unit Economics Calculator | [0077-unit-economics-calculator.md](0077-unit-economics-calculator.md) |
| 0078 | Agent Card Discovery Route | [0078-agent-card-discovery-route.md](0078-agent-card-discovery-route.md) |
| 0079 | Verification Store | [0079-verification-store.md](0079-verification-store.md) |
| 0080 | One-Way Inbox Notification | [0080-one-way-inbox-notification.md](0080-one-way-inbox-notification.md) |
| 0081 | File Chat Bridge | [0081-file-chat-bridge.md](0081-file-chat-bridge.md) |
| 0082 | OTLP/HTTP JSON Export | [0082-otlp-http-export.md](0082-otlp-http-export.md) |
| 0083 | MCP OAuth Token Exchange | [0083-mcp-token-exchange.md](0083-mcp-token-exchange.md) |
| 0084 | Skill Capture from a Completed Journal | [0084-skill-capture.md](0084-skill-capture.md) |
| 0085 | Skill Evaluation CLI Gate | [0085-skill-eval-cli-gate.md](0085-skill-eval-cli-gate.md) |
| 0086 | Benchmark Gate Metrics | [0086-benchmark-gate-metrics.md](0086-benchmark-gate-metrics.md) |
| 0087 | Text File Attachments | [0087-text-file-attachments.md](0087-text-file-attachments.md) |
| 0088 | Skill Canary CLI Gate | [0088-skill-canary-cli.md](0088-skill-canary-cli.md) |
| 0089 | CUA Single-Shot Plan | [0089-cua-single-shot-plan.md](0089-cua-single-shot-plan.md) |
| 0090 | Streaming Provider Deltas | [0090-streaming-provider.md](0090-streaming-provider.md) |
| 0091 | Parallel Read-Only Tools | [0091-parallel-read-only-tools.md](0091-parallel-read-only-tools.md) |
| 0092 | Journal Incremental Read | [0092-journal-incremental-read.md](0092-journal-incremental-read.md) |
| 0093 | Checkpoint Deltas | [0093-checkpoint-deltas.md](0093-checkpoint-deltas.md) |
