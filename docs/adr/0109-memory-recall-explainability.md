# ADR 0109: Memory Recall Explainability

- Status: accepted
- Date: 2026-11-12
- Related: SA97-17

## Context

Memory recall existed only as a library call. There was no way to ask why a
memory was returned, which source produced it, how trusted it is, or whether
it is still valid. SA97-17 requires cross-session retrieval that is
explainable.

## Decision

1. Add `1agent memory recall --query TEXT [--bot-id ID] [--limit N] [--json]`
   with the same journal/payload/key paths as the other memory commands.
2. Print each hit with its score, layer/kind, content, source kind and event
   id, taint, confidence, validity window and scope.
3. JSON output includes the query, the full scope object and each hit's full
   record plus score.
4. Empty queries fail closed; the limit is clamped to 1..=100. Recall is
   scope-bound, so a bot cannot see another bot's records.
5. `MemoryService::open` hydrates ciphertext payloads before recall, so the
   returned content is decrypted.

## Consequences

- A user can see why a memory was recalled and where it came from without
  reading the journal by hand.
- The output may contain decrypted memory content, including secrets. Do not
  pipe it to shared logs or paste it into a ticket.
- Recall remains substring/term scoring plus confidence/taint/utility; it is
  not semantic search and does not explain a model's use of the memory.
- Conflict display is not implemented; records are returned by score, not
  grouped as contradictory.
