# ADR 0026: Task-Graph Cross-Process Write Lock

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
   The writer does not auto-merge or retry; the caller must reopen the service
   and reapply the operation. This is fail-closed: a stale in-memory graph can
   never append after another writer.
4. Lock files are runtime artifacts, not state. They are empty and are not
   integrity-checked.
5. The guarantee is Linux/Unix only. On other platforms the lock is a no-op and
   only the byte-length check remains, which cannot close the race.
6. The task graph, memory journal, skill journal, goal log and action log all
   use the same lock-and-length-check append path. Memory payload files and
   cross-process indexes are still outside this guarantee.

## Consequences

- Two cooperating 1Agent processes can no longer corrupt the task-graph log;
  the loser fails with an actionable error.
- Concurrent operations are not merged. Live multi-process scheduling would
  need a reload/retry protocol and an index, neither of which is claimed here.
- A non-cooperating writer that ignores the lock can still race; the length
  check detects it only when it wins before the append.
