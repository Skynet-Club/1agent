# ADR 0055: Budget Policy and Cost per Verified Outcome

- Status: accepted
- Date: 2026-11-12

## Context

Cost governance needs a hard stop, not a dashboard number. The plan's M10
already reports cost per task; a multi-agent or long-horizon run also needs a
budget check that fails closed when tokens, cost or tool calls exceed a limit.

"Cost per task" rewards fast failure. The useful unit is cost per verified
outcome.

## Decision

1. Add `src/budget.rs` with:
   - `Budget { max_cost_micros, max_tokens, max_tool_calls }`, all optional;
   - `Usage { cost_micros, tokens, tool_calls }`;
   - `evaluate(usage, budget)` → `ok`, `near_limit` or `exceeded`.
2. Exceeding any configured limit is a failure (`allowed = false`). Reaching
   80% of a limit is a warning (`near_limit`, still allowed).
3. Unset limits are unbounded.
4. `cost_per_verified_outcome(cost, verified)` returns `None` for zero verified
   outcomes; a caller cannot turn fast failure into an infinite saving.
5. Add `1agent budget-check --cost-micros ... --tokens ... --tool-calls ...
   [--max-cost-micros ...] [--max-tokens ...] [--max-tool-calls ...]
   [--verified-outcomes ...] [--json]`. An exceeded budget exits non-zero.
6. This is a policy and CI helper. The runtime still needs to wire it into
   the run loop; that is a later slice.

## Runtime enforcement

7. `RunBudget` gains optional `max_cost_micros` and `max_tokens`; old
   serialized budgets default them to `None`.
8. The runtime accumulates provider-reported cost and input+output tokens
   after each model response. Exceeding either limit stops the run with
   `StopReason::BudgetExhausted` and a cost/token budget answer.
9. `1agent run --max-cost-micros` and `--max-tokens` set the run limits;
   `budget-check` remains the preflight/CI helper.

## Consequences

- A CI job or operator can gate a run on cost/tokens/tool calls.
- Near-limit warnings are visible before the hard stop.
- Cost per verified outcome is available as the primary economic metric.
- The runtime does not yet stop a live run on an exceeded budget.

## Evidence

```bash
cargo test budget --lib
cargo test --test budget_check_cli
```

The tests cover unbounded, under-limit, near-limit, exceeded and zero-verified
cases.
