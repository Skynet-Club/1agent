# Memory Recall Explainability — Evidence (2026-11-12)

Status: **passed on this machine**
Scope: `1agent memory recall` and scoped explainable hits.

## Command

```bash
cargo test --test memory_recall_cli
```

## What this proves

- A record stored in `bot-1` is returned with its content, source kind/event,
  taint, confidence, validity, layer/kind and score.
- The same query for `bot-2` returns no hits, so recall is scope-bound.
- An empty query exits non-zero.
- JSON includes the query, the full scope and each hit's full record plus
  score.

## What this does not prove

- Semantic retrieval; scoring is term/substring matching plus
  confidence/taint/utility.
- Conflict grouping; contradictory records are returned by score, not flagged.
- That a model used the recalled memory correctly.
- That output is safe to log; `MemoryService` hydrates decrypted content, so
  secrets can appear in the output.
