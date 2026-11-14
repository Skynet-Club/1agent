# Status v0.3.0

Date: 2026-11-01
Status: configurable provider fallback and semantic evidence contract

## Added since v0.2.0

- JSON provider configuration with multiple ranked candidates.
- API keys referenced by environment variable name; values are never stored in config.
- Retryable network, timeout, rate-limit and provider-unavailable failures fall through to the
  next route.
- A failed route is suppressed for the remainder of the same run to prevent repeated calls.
- Authentication, cancellation and invalid structured output do not trigger blind fallback.
- Deterministic context compaction prunes old tool-result bodies while preserving system text,
  the original task, recent messages and tool-call/result pairing.
- Structured `claims` are now part of every answer.
- Evidence entries can reference only declared claim ids.
- Semantic audit checks claim references and answer/stop-reason consistency.
- Hidden Unicode controls are rejected in both evidence files and experience candidates.

## Verification

| Check | Result |
|---|---|
| Unit tests | 29 passed |
| CLI run/audit/replay | passed |
| Config-file CLI run | passed |
| Experience lifecycle CLI | passed |
| OpenAI-compatible loopback fixture | passed outside managed sandbox |
| Linux release build | passed |
| `x86_64-pc-windows-gnu` all-target check | passed |
| Clippy `-D warnings` | passed |

Release artifact:

```text
size: 7.6 MiB
sha256: 73acc656cf1eb7902f0c5e77869f9eb4644ea04f852cd8bc3d5bcd4bbcd4002b
```

Config-file smoke:

```text
run_id: 4cb6ec3d-9685-45d1-924b-4e2169679532
events: 13
stop_reason: completed
unresolved_tool_calls: []
audit_issues: []
```

## Requirement comparison

| Requirement | Status |
|---|---|
| Multi-provider configuration | implemented |
| Runtime provider fallback | implemented; tested with deterministic failure injection |
| Context pruning | implemented for old tool-result bodies |
| Claim-level epistemic status | implemented |
| Semantic audit | implemented |
| Automatic candidate retrieval | pending |
| Held-out learning evaluation | pending |
| Run checkpoint/resume | pending |
| Real-provider quality | not verified |

## Known risks

- Fallback policy is static and quality scores are configured manually.
- Provider cost fields are metadata only; the runtime does not yet calculate actual cost.
- Context pruning is deterministic and lossy for old tool-result bodies; original data remains in
  the journal, but automatic retrieval from a checkpoint is not implemented.
- JSONL journals and experience stores remain single-writer.
- The Windows target has compile coverage only.

## Next requirement

Implement durable checkpoints and interrupted-run recovery:

1. Persist a reconstructable checkpoint after every completed tool batch.
2. Include canonical messages, cognitive mode, tool count and route state.
3. Resume only from a valid terminal-free journal prefix.
4. Never resume a completed, cancelled or semantically invalid run.
5. Journal the resume boundary and preserve original run id plus an explicit resume attempt id.
6. Add process-interruption equivalence tests proving resume produces the same observable answer.
