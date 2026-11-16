# 0.99 DK-001/002 — RunContext / parent lineage evidence

Date: 2026-11-16
Scope: `RunContext` task-local and parent session/run/ spawn-call/child-run
lineage for `spawn_agent`.
Result: **implemented and verified; DK-003/DK-004 remain open**. This is not a
0.99 GoalKernel acceptance and not a release.

All fixtures are synthetic and run in temporary directories. No production
journals, prompts, credentials or secrets are included.

## 1. Tested tree

| Field | Value |
|---|---|
| Repository | `/home/skynet/1-agent` |
| Branch | `0.99-dev` |
| Slice tag | `0.99-dk002-lineage` |
| Working tree | code + docs committed; only untracked 0.98.2 eval raw artifacts remain |
| Changed files | `src/tools/mod.rs`, `src/runtime.rs`, `src/tools/subagent.rs`, `src/subagent_registry.rs`, `src/main.rs`, `docs/architecture.md`, `docs/0.99-handoff.md` |

## 2. Commands and results

```bash
cargo test --offline --lib
./scripts/verify.sh
```

| Command | Result |
|---|---|
| `cargo test --offline --lib` | **1082 passed, 0 failed, 4 ignored** |
| `./scripts/verify.sh` | **passed** (fmt, clippy all-targets `-D warnings`, all tests, core-v3 8/8) |

Focused tests added or updated:

- `subagent_registry::tests::concurrent_appends_keep_one_parseable_record_per_line`
  — 8 threads × 50 appends; every line parses. Regression for the single
  `write_all` append fix.
- `tools::subagent::tests::registry_records_parent_session_run_and_spawn_call_lineage`
  — one spawn inside a run context: registry and observation metadata carry
  parent run, parent session, spawn call and child run.
- `tools::subagent::tests::concurrent_spawns_keep_their_parent_context` — two
  spawns joined concurrently with different run/session contexts; each record
  keeps its own lineage.
- `tools::subagent::tests::spawn_outside_a_run_context_keeps_parent_ids_empty`
  — no context means no fabricated parent ids.
- `runtime::tests::concurrent_runs_scope_their_own_run_context` — two runtimes
  with different sessions run concurrently; the tool observes the run id of
  its own run and the correct session id.
- Existing `subagent_registry::tests::legacy_records_without_lineage_fields_remain_readable`
  still passes (old registry rows stay readable).

## 3. Acceptance mapping

| DK-001/002 criterion | Evidence |
|---|---|
| Spawn record has `parent_session_id`, `parent_run_id`, `spawn_call_id`, `child_run_id` | Two lineage tests above; session id comes from the TUI session, run id from the creating run |
| Concurrent runs do not cross wires | Runtime concurrency test and spawn concurrency test |
| Members of different sessions are not attached to the wrong parent session | Each TUI run entry refreshes `set_session_id` immediately before the run |
| Child run and parent spawn observation are traceable both ways | Observation metadata has `child_run_id` and parent ids; registry has `parent_run_id`/`parent_session_id`/`spawn_call_id`/`child_run_id`. Child journal → parent link is DK-003 |
| Old registry rows readable | `legacy_records_without_lineage_fields_remain_readable` |
| Scoped tests and `verify.sh` green | Command table above |

## 4. Explicit non-claims

- `parent_session_id` is empty for `1agent run` and eval runs that are not in a
  TUI session; `parent_run_id` is still written. No session id is invented.
- Failure/timeout/cancel spawns do not have a `child_run_id`; that matrix is
  DK-004. The child journal's reverse reference to the parent spawn is DK-003.
- `RunContext` does not include `goal_id` yet; GK-002 adds it with the
  session/goal binding.
- The registry is UI/local state, not audit evidence; the run journal remains
  the audit source.
