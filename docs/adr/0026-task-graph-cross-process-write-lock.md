# ADR 0026: Cross-Process Journal and Snapshot Locks

- Status: accepted for the reliability slice
- Date: 2026-11-08

## Context

`TaskGraphService` replays an append-only JSONL log into memory and appends new
events with a contiguous sequence. Two processes opening the same log can both
believe they own the next sequence and interleave or duplicate events. The
store was explicitly documented as single-writer, but nothing enforced it.

## Decision

1. A sibling lock file `<store>.lock` is opened 0600 with `O_NOFOLLOW`; Unix
   callers take `flock(LOCK_EX)` for the whole read-check-write section.
2. `TaskGraphService::open` holds the lock while repairing a torn tail and
   replaying the log. `persist` holds it again, compares the file's byte length
   with the length observed at open/last persist, and only then appends.
3. A mismatch returns `TaskGraphStoreError::ConcurrentWrite` with both lengths.
   The service reloads the graph before returning the error and the Surface layer
   maps it to a retryable conflict, so the next request succeeds instead of the
   long-lived server staying wedged. The operation is not auto-merged; the
   caller retries it.
4. `open_read_only` stores reject `persist`/`with_graph_mut`, closing the hazard
   where an unlocked read followed by a stale append could pass the length check.
5. Opening a store does not change its parent directory's mode. Append still
   creates/chmods a private state parent as before.
6. Lock files are runtime artifacts, not state. They are empty and are not
   integrity-checked.
7. The guarantee is Linux/Unix only. On other platforms the lock is a no-op and
   only the byte-length check remains, which cannot close the race.
8. The task graph, memory journal, skill journal, goal log and action log all
   use the same lock-and-length-check append path.
9. Memory payload writes take a `<memory.key>.lock` file lock around ledger
   reload, scope-KEK creation, payload write/read/delete and key rotation. The
   keyring reloads the master bundle and ledger from disk under the lock, so two
   processes writing the first scope share one KEK instead of overwriting it.
10. Delegation and evolution JSON snapshots use the same sibling lock plus a
    SHA-256 comparison. A stale writer reloads the snapshot and returns a
    retryable conflict; the next request succeeds.
11. Cross-process indexes and automatic operation merging are still outside
    this guarantee.

## Consequences

- Two cooperating 1Agent processes can no longer corrupt the task-graph log;
  the loser gets a retryable error and the next request uses the reloaded state.
- Concurrent operations are not merged. Live multi-process scheduling would
  need an index and conflict-aware retry policy, neither of which is claimed.
- A non-cooperating writer that ignores the lock can still race; the length
  check detects it only when it wins before the append.
- Cross-process memory key creation is serialized and ledger-reloaded under the
  lock; the remaining cross-process gap is indexing and automatic merging.
- Snapshot locks protect whole-file replacement; they do not merge concurrent
  edits, so the loser must retry against the reloaded snapshot.
