# ADR 0111: Cascade Bench

- Status: accepted
- Date: 2026-11-12
- Related: SA99-61

## Context

`1agent cascade-check` evaluated one hand-written graph. SA99-61 asks for a
Cascade-Bench-style fault-injection benchmark: many scenarios with containment
points and a policy, summarized instead of checked one at a time.

## Decision

1. Add `CascadeScenario` (id, root, edges, contained nodes, policy) and
   `CascadeBenchReport` (scenarios/passed/failed, max depth, max blast radius,
   minimum containment, per-scenario results).
2. `parse_cascade_scenarios` reads JSONL and reports the source line; `bench`
   validates ids, duplicates, root/edge bounds, scenario count (≤1000) and
   `min_containment_bps <= 10000`.
3. `1agent cascade-bench --input PATH [--json]` reads at most 10 MiB, prints
   the report, and exits non-zero when any scenario fails its policy.
4. Each scenario is still a deterministic simulation; containment stops BFS
   propagation at marked nodes.

## Consequences

- A containment policy can be regression-tested over a fixture set instead of
  one ad-hoc graph per invocation.
- The benchmark is synthetic. It does not run real agents, inject real faults
  or measure recovery time; it measures graph containment semantics.
- A scenario that fails identifies the scenario id and the policy failures.
- This is not the 24h multi-agent acceptance in SA99; it is a deterministic
  predecessor.
