# Architecture

Status: accepted for V0

## Goals

The runtime must be cross-platform, provider-neutral, recoverable, auditable and safe by default. The domain core owns cognition state, evidence, policy, stop reasons and the run journal contract. Hosts and providers are adapters.

## Layers

```text
CLI host
  -> application service
    -> agent runtime
      -> model router -> provider adapter
      -> tool pipeline -> read-only tools
      -> run journal -> JSONL store
```

## V0 decisions

- Rust 2024 and Tokio for a portable single-binary core with explicit cancellation and timeouts.
- JSONL append-only run journal as the first durable fact store.
- Canonical internal message, tool-call, observation and answer types.
- OpenAI-compatible HTTP adapter is only a provider adapter, not the internal protocol.
- Deterministic fake provider is required for offline tests and evaluation.
- Read-only file tool is the first real capability.
- No UI, database, vector store, agent framework or online self-modification in V0.

## Implemented topology

```text
first-principles-agent CLI
  -> AgentRuntime
    -> deterministic mode inference
    -> static capability router
    -> DeterministicProvider or OpenAiCompatibleProvider
    -> read-only file/search tools
    -> context budget and durable checkpoints
    -> Journal / JSONL
    -> replay, semantic audit and offline evaluation
    -> versioned scorecards -> content digest -> optional approval envelope
    -> experience lifecycle and held-out promotion gates
    -> stdio host protocol

release pipeline
  -> two isolated builds per target
  -> normalized package tree
  -> deterministic tar.gz and zip
  -> build provenance and complete SHA256SUMS
  -> external release manifest
  -> optional approval and production snapshot-chain verification
```

The V0 runtime runs in the CLI process. Application boundaries are defined so that an
independent local service can replace the in-process host later without changing domain
events or provider/tool contracts.

## Implemented contracts

- `TaskRequest`: objective, deliverable, known facts, constraints, unknowns, decision and
  success criteria, permissions and budgets.
- `AgentAnswer`: result status, conclusion, evidence, assumptions, constraints, unknowns,
  flip conditions and action receipts.
- `JournalEvent`: protocol version, run id, contiguous sequence, timestamp and typed event.
- `ModelRequest` and `ModelResponse`: canonical provider-neutral messages and tool calls.
- `ToolObservation`: structured status, content, error code and metadata.
- `EvalReport`: suite outcome counts, decision-completeness policy version/source,
  canonical content digest and per-case results.
- `EvalReportSummary`: case-content-free status view with schema version, integrity,
  content-binding, signature-presence and authenticity states.
- `ApprovalEnvelope`: HMAC-authenticated binding to exact report bytes, constrained by
  approval key registry state.

## Security behavior

- Read access is disabled without configured roots.
- Canonicalized file paths must remain below a configured read root.
- Write authority is false by default.
- Execute-mode requests stop at `needs_authorization` without calling a provider.
- Provider network access requires explicit `--allow-network`.
- API keys are read from an environment variable or hidden CLI option and are not journaled.
- Non-loopback provider credentials require HTTPS; plaintext HTTP is limited to loopback development.
- Release verification fails closed on dirty source, missing artifact roles, digest drift,
  incomplete checksums, invalid approval kind, revoked keys or unanchored registry history.
- Report inspection always validates policy compatibility and internal aggregate consistency.
- Content-bound reports detect payload mutation by recomputing `content_sha256`.
- Authenticated report inspection requires an `evaluation_report` approval envelope and
  Active/Retired, non-revoked registry key; unauthenticated status is never upgraded implicitly.

## V0 limitations

- Proving real-model quality without configured providers.
- Calling Linux commands or writing files.
- Streaming model output.
- Multi-agent orchestration.
- Modifying Skynet Terminal.
- Asymmetric release signatures; current approval is HMAC-based.
- Asymmetric report signatures; report authentication is currently HMAC-based.
- Cross-machine hermetic toolchain reconstruction.
- macOS verification and Windows runtime execution.

## Evaluation trust boundary

The report trust chain separates four claims:

1. **Integrity checked**: policy metadata, arithmetic, aggregates and optional digest passed.
2. **Content binding**: the report contains a canonical digest of its own payload.
3. **Signature present**: an external approval envelope was supplied and verified.
4. **Authenticity verified**: signature verification succeeded for the configured key registry.

These claims do not establish evaluation truth or model reasoning quality. Unsigned reports
remain inspectable for diagnostic use, while hosts may require content binding or authenticity.
See `docs/adr/0004-evaluation-report-trust.md`.

## Cross-platform rules

- Domain and application code must not branch on `cfg(target_os)`.
- Platform capabilities belong in adapters with explicit `verified`, `degraded`, `unsupported` and `untested` status.
- File access is constrained to canonicalized read roots.
- Cancellation must propagate through provider calls, tools and cleanup.
- Structured errors are part of the contract; provider-specific error text is not.
