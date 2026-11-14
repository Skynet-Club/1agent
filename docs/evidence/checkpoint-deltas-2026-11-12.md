# Checkpoint Deltas — Evidence (2026-11-12)

Status: **passed on this machine**
Scope: `RuntimeCheckpointDelta`, `CheckpointDelta`, resume/replay merging,
audit and the runtime writer.

## Commands

```bash
cargo test checkpoint_delta --lib
cargo test delta_checkpoints_keep_the_journal_linear --lib -- --ignored --nocapture
```

## Results

```text
checkpoint_delta_extends_the_previous_checkpoint ... ok
checkpoint_delta_with_a_wrong_base_is_rejected ... ok
audit_flags_an_orphan_checkpoint_delta ... ok
checkpoint_deltas_resume_to_the_same_answer ... ok

journal total=3299671 checkpoint actual=114920 full-checkpoint estimate=2986785 ratio=0.038
```

## What this proves

- A journal with one full checkpoint followed by two deltas resumes to the
  same stop reason, conclusion and evidence as the uninterrupted run.
- A delta's message list and cumulative fields are merged into the base
  checkpoint in order.
- A delta whose `base_seq` is not the previous checkpoint event is rejected
  with `InvalidCheckpointDelta`.
- Audit reports an orphan delta with `OrphanCheckpointDelta`.
- On a 100-step echo run, checkpoint events total 115 KB; writing full
  checkpoints at each step would have written about 2.99 MB (3.8%).

## What this does not prove

- Total journal size is still dominated by `ModelRequested`; this slice does
  not reduce that per-attempt full request copy.
- Compatibility with an older binary reading new journals; an unknown
  `checkpoint_delta` variant is an error, so downgrades need a fresh journal.
- Message-history mutation other than append; the writer falls back to a full
  checkpoint only when the history is shorter than the checkpointed length.
- Real-provider or 1M-context sizes; the probe uses a deterministic echo run.
