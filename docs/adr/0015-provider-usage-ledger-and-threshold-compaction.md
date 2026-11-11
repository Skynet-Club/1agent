# ADR 0015: Provider Usage Ledger and Threshold Compaction

Status: accepted; context threshold superseded in part by ADR 0034

Date: 2026-11-03

## Context

> Context budgeting and the compaction threshold are now token-based; see ADR 0034.

Character headroom alone does not answer how many tokens were consumed or what the
configured price metadata implies. Waiting until the hard context limit also causes
avoidable prompt growth before compaction occurs.

## Decision

Every `ModelResponded` event already records a provider-neutral `Usage` with input
tokens, output tokens and `cost_micros`.

- If the provider reports a non-zero cost, preserve it.
- If it reports zero cost, calculate cost from the selected route manifest:
  `ceil(input_tokens * input_cost_micros_per_k / 1000) +
   ceil(output_tokens * output_cost_micros_per_k / 1000)`.
- Apply the calculation before journaling both normal and repair responses.

`/cost` rebuilds a bounded session ledger from run journals and shows:

- call count;
- input/output/total tokens;
- configured `cost_micros`;
- zero-cost calls;
- provider/model breakdown;
- latest per-run breakdown.

The ledger does not include external Workshop Agent processes that do not write a 1A
run journal, and `cost_micros` is not labeled as a specific currency.

Context compaction now triggers at a configurable percentage of the maximum context
characters instead of waiting for the hard limit:

- default: 80%;
- accepted range: 50%–95%;
- configurable with `/config set compact <percent>`;
- persisted in session preferences;
- journaled as the existing bounded `ContextCompacted` event.

## Consequences

- Cost visibility becomes derived from durable journal facts rather than an in-memory UI
  counter.
- A provider's own reported cost remains authoritative when available.
- Zero-cost local providers remain distinguishable through the zero-cost call count.
- Automatic compaction creates more headroom and reduces risk of a late context collision.
- Character headroom and token/cost accounting remain separate concepts.

## Verification

- manifest pricing is applied when provider usage omits cost;
- provider-reported non-zero cost is preserved;
- ledger aggregation covers input, output, cost, providers and runs;
- auto-compaction triggers before the hard character limit;
- TUI `/cost`, `/context show`, `/status` and `/config compact` are covered;
- legacy v2 preferences without the new threshold field load with the 80% default.
