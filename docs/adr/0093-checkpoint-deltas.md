# ADR 0093: Checkpoint Deltas

- Status: accepted
- Date: 2026-11-12

## Context

The runtime wrote a full `RuntimeCheckpoint` after every tool batch. The
checkpoint carries the entire message history, so a long run wrote the growing
history again at every step (O(n²) bytes in checkpoint events). The journal
must stay replayable and resume must reconstruct exactly the same state.

## Decision

1. Add `RuntimeCheckpointDelta` and the `CheckpointDelta` journal event. A
   delta carries `base_seq` (the seq of the event that established the base
   checkpoint) plus only the messages appended since that checkpoint, along
   with the cumulative counters and route metadata.
2. The runtime writes a full `Checkpoint` for the first checkpoint and deltas
   afterwards. If the message history is ever shorter than the checkpointed
   length, it falls back to a full checkpoint.
3. `find_resumable_prefix` and `replay_events` apply deltas in order; a delta
   whose `base_seq` is not the previous checkpoint event is rejected with
   `InvalidCheckpointDelta`, and a delta with no base fails with
   `MissingCheckpointBase`. A full checkpoint or `ResumeStarted` resets the
   base, so old journals (full checkpoints only) and resumed journals keep
   working unchanged.
4. `audit_events` reports an orphan delta whose base is not the previous
   checkpoint.
5. `ResumeStarted` still embeds a full reconstructed checkpoint, so a resumed
   journal has a self-contained base.

## Consequences

- Checkpoint bytes become linear in the number of messages instead of
  quadratic. On a 100-step echo run, checkpoint events are 115 KB instead of a
  ~2.99 MB full-checkpoint estimate (3.8%).
- Total journal size is still dominated by `ModelRequested`, which continues
  to serialize the full model-visible request for every provider attempt.
  Reducing that is remaining SA97-11 work and is not claimed here.
- A journal containing `checkpoint_delta` cannot be read by an older binary
  that does not know the variant. Downgrades must finish or archive new
  journals first; replay of existing old journals is unaffected.
- Delta application is strict: a mismatched base fails closed rather than
  producing a plausible but wrong message history.
