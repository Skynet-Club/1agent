# Status v0.4.0

Date: 2026-11-01
Status: durable checkpoint and interrupted-run recovery

## Added since v0.3.0

- A checkpoint after every complete tool-call batch.
- Checkpoint contents: cognitive mode, canonical messages, tool-call count, elapsed budget,
  failed-provider suppression and next step.
- `resume` runtime API and CLI command.
- Resume journals record original run id, source checkpoint sequence and resume attempt id.
- Resume rejects:
  - empty journals;
  - non-contiguous sequence or changed run identity;
  - unpaired tool calls and observations;
  - completed, cancelled or otherwise terminal runs;
  - unsafe execute-mode prefixes;
  - runs without a checkpoint.
- Resume preserves the same task, messages, evidence and tool observations.

## Verification

| Check | Result |
|---|---|
| Unit tests | 30 passed |
| CLI run/audit/replay | passed |
| Config-file CLI run | passed |
| Experience lifecycle CLI | passed |
| Interrupted-run runtime equivalence | passed |
| Interrupted-run CLI equivalence | passed |
| Completed-run resume rejection | passed |
| Clippy `-D warnings` | passed |

## Requirement comparison

| Requirement | Status |
|---|---|
| Durable checkpoint | implemented after tool batches |
| Failed-route memory across resume | implemented |
| Original and resume identities | implemented |
| Resume safety gates | implemented |
| Context/content compaction | deterministic tool-body pruning implemented |
| Automatic experience retrieval | pending |
| Held-out learning evaluation | pending |
| Real-provider quality | not verified |

## Known risks

- An interruption during a tool batch, before the checkpoint, requires rerunning that batch.
- Resuming starts a new timeout window but carries prior elapsed time into the remaining budget;
  wall-clock overhead during downtime is intentionally excluded.
- Checkpoint messages include tool observations and can be large; encrypted storage and retention
  policy are pending.
- A resumed run creates a new journal and does not append to the interrupted file.

## Next requirement

Implement controlled experience retrieval and injection:

1. Retrieve only `active` candidate records.
2. Filter by mode, scope and explicit applicability.
3. Mark injected experience as unverified policy guidance, not fact.
4. Cap the amount injected under the context budget.
5. Journal every selected candidate and its version.
6. Never inject rejected, rolled-back or merely approved candidates.
7. Add negative tests for stale and out-of-scope lessons.
