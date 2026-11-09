# ADR 0018: Deterministic Global Scheduling Across Master Conversations

- Status: accepted
- Date: 2026-11-04

## Context

Multiple Master conversations must share machine capacity, Agent availability and workspace
write safety. Letting each Master independently dispatch up to its plan parallelism would
overcommit resources and could start overlapping writes in the same repository.

## Decision

1. Scheduling is deterministic and owned by code, never by a planning model.
2. Work Items use a globally stable identity `<conversation_id>:<task_id>`.
3. Each conversation has an independent active-task limit.
4. A global active-task limit is applied after per-conversation eligibility.
5. Ready tasks are selected round-robin across conversations for fairness.
6. A dependency unlocks only when its status is `verified`.
7. Write-bearing tasks with equal, parent or child workspace paths are serialized.
8. The scheduler emits explicit start and waiting decisions with bounded reasons:
   dependency, global capacity, conversation capacity, write conflict or not ready.
9. The first implementation is a pure local contract and CLI. Persisted execution leases
   remain the authority before a real Session starts.

## Rejected alternatives

- Global FIFO: one large plan can starve another conversation.
- Pure plan parallelism: causes machine and workspace overcommit.
- Model-selected ordering: non-deterministic and hard to audit.
- Treating `ArtifactReady` as dependency success: acceptance would bypass verification.

## Consequences

- The scheduler can be unit-tested without Workshop or a model.
- Workspace paths are conservatively treated as overlapping when one contains the other.
- Future multi-Master integration must pass current task states and workspace reservations
  into this scheduler before calling a Workshop execution adapter.
