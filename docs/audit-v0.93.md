# v0.93.0 Master-Agent Audit

Date: 2026-11-03

## Verdict

The bounded Master-Agent V1 orchestration loop is implemented and verified. This verdict
covers planning, approval, assignment, Session execution, supervision, retry/replan,
independent Artifact verification, Journal replay and Workshop compatibility.

The verdict does not claim measured success rates for Pi, Hermes, Claude or Codex. Their
same-task probes were blocked by local authentication, model-selection or network
conditions and remain recorded as blocked.

## Requirement matrix

| Requirement | Implementation | Verification |
|---|---|---|
| Direct vs single vs multi-step/multi-Agent triage | `master_planner::triage_goal` | direct/single/multi tests |
| Natural-language Work Item DAG | `ModelPlanGenerator` + strict Plan parser | fake-provider test and live DeepSeek plan |
| Bounded DAG/budget/permission/Artifact checks | `PlanProposal::validate` | cycle, path, blank field and limit tests |
| Capability/tool/risk/write assignment | `PlanProposal::assign` | hard-filter, quality and accumulated-load tests |
| Plan approval, reject and revision | `MasterState::{approve,reject_plan,revise_plan}` | unstarted-plan lifecycle test |
| Model cannot self-approve or broaden paths | strict parser + read-root/workspace checks | identity/self-approval/path tests |
| Session mapping and durable lease | Coordinator + Workshop Adapter | sync and detached CLI E2E |
| Idempotency and duplicate prevention | generation + idempotency key + active lease guard | duplicate receipt and expired/re-lease tests |
| Parallel dispatch | scoped Worker dispatch up to `max_parallel` | CLI E2E and fake Adapter test |
| Running / waiting / blocked / timeout | detached Session poll + explicit tail markers | real running->finished; timeout and marker tests |
| Waiting-input marker protocol | bounded last-marker parser | marker precedence test |
| User takeover / cancel / pause | Coordinator state + Session kill | lifecycle methods and CLI cancel path |
| One bounded retry | `request_retry` | retry-limit test |
| One affected-subgraph replan | `request_replan` / `apply_replan` | bounded replan and preservation tests |
| Artifact existence and SHA-256 | `inspect_artifacts` / verifier | tamper and missing-Artifact tests/pilot |
| No completion without evidence | verifier transition gate | false-success real Workshop pilot |
| Journal replay and tamper audit | chained JSONL state events | chain, replay and concurrent-writer tests |
| Planner input/response audit | 0600 planning JSONL | repair/trace test and live planner journal |
| Workshop Agent/Job/event reads | bounded CLI Adapter | unit tests plus 13 Agents / 7 Jobs live read |
| Independent use without Workshop | plan, validate and assign commands | offline CLI tests |

## Automated gates

```text
cargo fmt --all -- --check                  passed
cargo test --all-targets                    passed
cargo clippy --all-targets -- -D warnings   passed
RUSTDOCFLAGS='-D warnings' cargo doc         passed
```

Test totals:

```text
library: 187 passed
main/TUI: 4 passed
CLI E2E: 8 passed
other integration suites: all passed
loopback-only provider test: ignored by design in managed sandbox
```

## Real Workshop gates

Final release-binary controlled execution:

```text
trace: trace-final-bounded-20261103
session: term-1793636352398-530889-18
result: verified
audit: 7 events, hash chain valid
```

Detached supervision and terminal convergence:

```text
trace: trace-live-running2-20261103
session: term-1793635550991-530889-15
observed running -> finished -> artifact_ready -> verified
audit: 8 events, hash chain valid
```

False-success pilot:

```text
trace: trace-pilot-missing-20261103
exit code: 0
verification result: rejected
reason: missing.txt is absent
audit: 6 events, hash chain valid
```

## Workshop regression gate

```text
cargo fmt --all -- --check                 passed
cargo clippy --workspace --locked ...      passed
cargo test --workspace --locked (skip loopback) passed
```

Observed suite groups included 39 `skynet-core`, 109 GUI library, 29 GUI application,
11 jobd, 19 headless runner and all remaining integration tests. The only skipped test
binds a loopback socket, which the managed sandbox denies with `EPERM`.

## Residual limits

- Four runtime success rates remain unmeasured because their same-task probes were
  blocked. Declared capabilities are not promoted to observations.
- Job execution remains observable and mapped by ID, while controlled Work Item dispatch
  currently uses persistent Sessions. Job dispatch requires a separate permission-equivalent
  design before it should be enabled.
- Human baseline data is currently the deterministic false-success pilot, not a production
  productivity study. No speed or success-rate improvement is claimed.
- The workspace is intentionally uncommitted and untagged; this audit does not alter the
  user repository history.
