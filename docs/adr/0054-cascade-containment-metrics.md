# ADR 0054: Cascade Containment Metrics

- Status: accepted
- Date: 2026-11-12

## Context

Multi-agent systems fail through interaction: one worker's bad result reaches a
parent, then siblings, then the final report. The plan's 0.99 gate needs a
deterministic way to measure whether containment stops propagation before
fan-out grows.

There is no live multi-agent cascade harness yet. This slice defines the metric
contract and a deterministic simulation.

## Decision

1. Add `src/cascade.rs`.
2. `simulate(root, edges, contained)` walks a directed failure graph:
   - the root is affected;
   - a node marked `contained` is affected but does not propagate;
   - uncontained nodes propagate to their children.
3. The report contains:
   - `affected`: all reached nodes;
   - `contained`: affected nodes that stopped propagation;
   - `propagated`: affected nodes that did not;
   - `containment_rate_bps = contained / affected`;
   - `cascade_depth`: maximum depth reached (root = 0);
   - `blast_radius = affected.len()`.
4. `evaluate(report, policy)` fails when containment is below the minimum,
   depth exceeds the maximum or blast radius exceeds the maximum.
5. Add `1agent cascade-check --root NODE --edge PARENT>CHILD --contain NODE
   [--min-containment-bps ... --max-depth ... --max-blast-radius ...]
   [--json]`.
6. This is a simulation and policy gate, not a live multi-agent run. The 0.99
   gate still needs a real 24h multi-agent task.

## Consequences

- A team can test containment rules before enabling more fan-out.
- The metrics are deterministic and easy to compare across runs.
- A high containment rate with a small blast radius is visible separately from
  task success.
- The simulation does not model correlated model errors or timing; those need
  a live harness.

## Evidence

```bash
cargo test cascade --lib
cargo test --test cascade_check_cli
```

The tests cover containment at a middle node, no containment, a contained root
and policy failures.
