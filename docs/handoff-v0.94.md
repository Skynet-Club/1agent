# Handoff v0.94.0

Date: 2026-11-03

## State

The repository is at version `0.94.0` with the phased 1A production-control command system
implemented. The worktree is intentionally dirty and uncommitted; no tag or history rewrite
was made.

Release binary:

```text
target/release/first-principles-agent
sha256 864ffaf035af9e205a09a875ddb75319e13709bc009a7a08d3cf6bbfe52f606b
```

Primary modules:

```text
src/tui_commands.rs             typed Core/Master command registry
src/tui_host.rs                 explicit host binding
src/tui_session.rs              durable session, mission, preferences and context summary
src/tui_first_principles.rs     deterministic evidence/report/audit views
src/tui_master_verification.rs  Artifact, Master evidence/report/audit views
src/tui_workspace.rs            presets, Git helpers and safe AGENTS.md initialization
src/master.rs                   planning and assignment contracts
src/master_coordination.rs      lease/state/verification journal
src/master_workshop.rs          bounded Workshop adapter
src/main.rs                     CLI/TUI controllers
```

## Verify

```bash
cargo fmt --all -- --check
cargo test --all-targets
cargo clippy --all-targets -- -D warnings
RUSTDOCFLAGS="-D warnings" cargo doc --no-deps
cargo build --release
```

## Production TUI flow

```text
/mission new <goal>
/plan generate
/approve
/agents refresh
/assign apply
/dispatch detach
/watch
/attention
/respond <task-id> <message>
/artifacts
/verify <task-id> <verification.json>
/evidence master
/audit
/report
```

## Cost controls

```text
/context show
/context compact
/config set context 30000
/config set attempts 3
/config set compact 80
/cost
```

Cross-runtime capability evidence:

```bash
1agent master capability-probe \
  --manifest configs/capability-probes.example.json \
  --output capability-report.json
```

This invokes external runtimes and may consume provider quota.

These controls use character budgets, not exact provider tokens. (Historical; context
budgets are token-based since ADR 0034.)

`/cost` is separate from character headroom: it rebuilds provider usage from run journals
and applies configured route pricing metadata when the provider does not report cost.

## Known next work

- Add automatic threshold-based compaction after evaluating false-summary risk.
- Re-run authenticated multi-runtime capability and external Workshop acceptance on the
  target host before claiming fresh production success rates.
- Build a production human-workflow baseline; the current evidence is technical acceptance,
  not productivity proof.
- Avoid non-production UI decoration unless it directly improves operator safety or
  observability.
