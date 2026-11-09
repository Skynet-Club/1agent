# Handoff v0.93.0

Date: 2026-11-03

## State

The repository is at version `0.93.0` with the Master-Agent V1 loop implemented. The
working tree is intentionally dirty and uncommitted. No tag or history rewrite was made.

Primary modules:

```text
src/master.rs                 plan, Work Item, assignment and verification contracts
src/master_planner.rs         triage, model planning, strict extraction and planner Journal
src/master_coordination.rs    approval, leases, state machine, retries/replans and audit
src/master_workshop.rs        bounded Workshop reads, sync/async Session execution
src/main.rs                   `master` CLI family
```

## Verify

```bash
cargo fmt --all -- --check
cargo test --all-targets
cargo clippy --all-targets -- -D warnings
RUSTDOCFLAGS='-D warnings' cargo doc --no-deps
```

## Minimal standalone flow

```bash
1agent master triage --task "先审计，然后修复，最后测试" --json
1agent master validate --plan plan.json
1agent master assign --plan plan.json --agents agents.json --json
```

## Minimal Workshop flow

```bash
1agent master init --plan plan.json --agents agents.json --trace-id trace-1
1agent master approve --trace-id trace-1 --actor user
1agent master run --detach --trace-id trace-1 \
  --runtime-profiles agents.json --workshop-prefix skynet-workshop
1agent master poll --trace-id trace-1 --json
1agent master verify --trace-id trace-1 --input verification.json
1agent master audit --trace-id trace-1 --json
```

## Known next work

- Run the same capability suite on authenticated, network-enabled hosts and update measured
  success/latency/cost fields.
- Run a production-like human baseline study; the current pilot covers false-success
  detection only.
- Design permission-equivalent Job dispatch if Job execution is needed as a first-class
  Work Item mode.
- Keep the Workshop compatibility boundary: platform DTOs tolerate additive fields; 1A's
  own safety/state contracts remain strict.
