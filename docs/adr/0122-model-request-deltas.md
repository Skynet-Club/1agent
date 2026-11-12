# ADR 0122: Model Request Deltas

- Status: accepted
- Date: 2026-11-12
- Related: SA97-11

## Context

`ModelRequested` carried the full model-visible request on every provider
attempt. The request contains the whole message history, so a long run wrote
the growing history again at every step (O(n²) bytes). Checkpoint deltas
(ADR 0093) removed the same pattern from checkpoints; the remaining journal
size was dominated by model requests.

The journal must stay replayable and every model-visible input must remain
reconstructable from it.

## Decision

1. Add `ModelRequestDelta` with `base_seq` (the previous request event), a
   `prefix_len`, the messages after that prefix, and `tools` only when the tool
   list changed.
2. `ModelRequestLog` writes a full `ModelRequested` for the first request in a
   journal and writes deltas afterwards. If `run_id` or `mode` changes, or no
   previous request exists, it writes a full request again.
3. The delta is computed against the previous request, not the raw message
   history: the longest equal message prefix is kept and the rest is stored.
   Compaction or tool-list changes therefore produce a correct, larger delta
   rather than an invalid one.
4. `ModelRequestChain` rebuilds the request in order. A delta with no base, a
   mismatched base, or a prefix longer than the base fails closed.
5. `audit_events` reports `orphan_model_request_delta` and
   `invalid_model_request_delta`. `replay_events` still ignores request
   content because replay is structural and does not need the prompt text.
6. TUI trace/snapshot, stage latencies, the usage ledger and OTel spans treat
   both event kinds as a request boundary; TUI token counts use the
   reconstructed request.
7. A resumed journal starts with no previous request, so its first request is
   full and the chain resets naturally.

## Consequences

- On the 100-step echo run, model-request events are 74,911 bytes instead of
  the 3,028,246-byte full-request estimate (2.5%). The whole journal is
  329,465 bytes.
- A journal containing `model_request_delta` cannot be read by an older binary
  that does not know the variant. Downgrades must finish or archive new
  journals first; replay of existing old journals is unaffected.
- Consumers that need the prompt must use `ModelRequestChain`; reading only
  `ModelRequested` now sees the first request only.
- The measurement covers journal bytes on a deterministic echo run. It does
  not measure provider wire bytes, CPU time, or real-model transcripts.
