# Cascade Bench — Evidence (2026-11-12)

Status: **passed on this machine**
Scope: `CascadeScenario` JSONL, `bench` and `1agent cascade-bench`.

## Commands

```bash
cargo test cascade --lib
cargo test --test cascade_bench_cli
```

## What this proves

- A contained scenario (`a→b→c`, containment at `b`, min 5000 bps, depth 1,
  radius 2) passes and reports 5000 bps.
- An uncontained scenario fails its policy, the command exits non-zero and
  the report records `failed=1`.
- The aggregate reports max depth, max blast radius and minimum containment.
- Duplicate ids, an empty root, `min_containment_bps > 10000` and an empty
  scenario list fail closed.
- A malformed JSONL line reports its line number.

## What this does not prove

- Real fault injection, agent execution or recovery time.
- The 24h multi-agent acceptance in SA99.
- That containment points reflect a real system's trust boundaries; the
  operator supplies the graph.
