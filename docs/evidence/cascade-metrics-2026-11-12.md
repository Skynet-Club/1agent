# Cascade Containment Metrics — Evidence (2026-11-12)

Status: **passed on this machine**  
Scope: `cascade::simulate`, `cascade::evaluate` and `1agent cascade-check`.
This is a deterministic simulation, not a live multi-agent run.

## Commands

```bash
cargo test cascade --lib
cargo test --test cascade_check_cli
cargo clippy --all-targets -- -D warnings
```

## Expected results

```text
running 4 tests
test cascade::tests::containment_stops_propagation ... ok
test cascade::tests::no_containment_reaches_the_whole_chain ... ok
test cascade::tests::contained_root_limits_the_blast_radius ... ok
test cascade::tests::policy_fails_closed_on_depth_or_blast_radius ... ok

test result: ok. 4 passed; 0 failed

running 3 tests (tests/cascade_check_cli.rs)
test containment_at_b_stops_the_chain ... ok
test no_containment_fails_the_default_policy ... ok
test malformed_edge_fails_closed ... ok

test result: ok. 3 passed; 0 failed
```

## What this proves

- Containing `b` in `a→b→c→d` limits affected nodes to `a,b`, depth 1 and
  blast radius 2; containment rate is 5000 bps.
- Without containment the whole chain is affected and the default policy
  fails.
- A contained root limits the blast radius to the root.
- Policy failures are explicit and fail closed.
- The CLI parses `PARENT>CHILD` edges and rejects malformed input.

## What this does not prove

- That a real multi-agent run contains failures the same way; no live harness
  was run.
- That model errors are independent; the simulation assumes deterministic
  propagation.
- That 24h multi-agent acceptance passes; the 0.99 gate is still open.
- Cross-platform behavior; the tests ran on Linux only.

## Files

- `src/cascade.rs`
- `src/main.rs` (`cascade-check`)
- `tests/cascade_check_cli.rs`
- `docs/adr/0054-cascade-containment-metrics.md`
- `docs/architecture.md`, `docs/features.md`, `docs/features.en.md`,
  `docs/claims.md`, `CHANGELOG.md`
