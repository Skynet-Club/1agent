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
    -> static Router
    -> DeterministicProvider or OpenAiCompatibleProvider
    -> ToolRegistry / ReadFileTool
    -> Journal / JSONL
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

## Security behavior

- Read access is disabled without configured roots.
- Canonicalized file paths must remain below a configured read root.
- Write authority is false by default.
- Execute-mode requests stop at `needs_authorization` without calling a provider.
- Provider network access requires explicit `--allow-network`.
- API keys are read from an environment variable or hidden CLI option and are not journaled.

## V0 limitations

- Proving real-model quality without configured providers.
- Calling Linux commands or writing files.
- Streaming model output.
- Multi-agent orchestration.
- Modifying Skynet Terminal.
- Provider fallback beyond capability routing.
- Long-term memory, skill promotion and context compaction.
- macOS and Windows build/runtime verification.

## Cross-platform rules

- Domain and application code must not branch on `cfg(target_os)`.
- Platform capabilities belong in adapters with explicit `verified`, `degraded`, `unsupported` and `untested` status.
- File access is constrained to canonicalized read roots.
- Cancellation must propagate through provider calls, tools and cleanup.
- Structured errors are part of the contract; provider-specific error text is not.
