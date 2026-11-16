# 0.99 DK-003/004 — child reverse link and failure matrix evidence

Date: 2026-11-16
Scope: child journals record the parent run/spawn call; failed, cancelled and
child-reported stop reasons carry structured lineage metadata to the parent;
the registry status matches.
Result: **implemented and verified; the parent hard-timeout branch is exercised
by code path but not by a wall-clock test, and ontology SquadMember nodes are
still DK-005.**

All fixtures are synthetic and live in temporary directories. No production
journals, prompts, credentials or secrets are included.

## 1. Tested tree

| Field | Value |
|---|---|
| Repository | `/home/skynet/1-agent` |
| Branch | `0.99-dev` |
| Slice tag | `0.99-dk004-failure-matrix` |
| Changed files | `src/journal.rs`, `src/runtime.rs`, `src/main.rs`, `src/tools/mod.rs`, `src/tools/subagent.rs`, `src/subagent_registry.rs`, `src/tui_first_principles.rs`, `tests/browser_approvals_cli.rs`, `docs/architecture.md`, `docs/0.99-handoff.md` |

## 2. Commands and results

```bash
cargo test --offline --lib
./scripts/verify.sh
```

| Command | Result |
|---|---|
| `cargo test --offline --lib` | **1111 passed, 0 failed, 4 ignored** |
| `./scripts/verify.sh` | **passed** (fmt, clippy all-targets `-D warnings`, all tests, core-v3 8/8) |

Focused tests added:

- `runtime::tests::child_run_journal_records_the_parent_link` — a run with a
  parent link writes `ParentRunLinked` and replay exposes
  `parent_run_link`.
- `tools::subagent::tests::child_receives_parent_lineage_environment` — the
  child process receives the parent run/session/spawn-call environment.
- `tools::subagent::tests::failed_spawn_observation_keeps_parent_lineage` —
  non-zero child exit yields `FailedWithMetadata` with parent ids, spawn call,
  workspace and exit status; the registry record is `Failed` with the same
  lineage.
- `tools::subagent::tests::parent_rejects_a_child_that_did_not_complete` —
  child-reported non-completed stop reasons are rejected with status,
  child run id and stop reason metadata.
- `tools::subagent::tests::cancelled_spawn_is_structured_and_marked_cancelled`
  — cancellation returns `CancelledWithMetadata` and the registry record is
  `Cancelled`.
- Legacy journal replay tests still pass; `ParentRunLinked` is additive and
  optional in `ReplayState`.

## 3. Acceptance mapping

| DK-003/004 criterion | Evidence |
|---|---|
| Child journal points back to the parent | Env injection + `ParentRunLinked` + replay test |
| Parent and child are traceable both ways | Registry parent ids (DK-002) + child journal link + observation metadata |
| Failures/timeouts/cancels are structured, not completed | Failure and cancellation tests; status metadata and registry state |
| Fanout/depth negatives still pass | Existing tests unchanged and green |
| Old registry/journals readable | Additive serde defaults; full suite green |

## 4. Explicit non-claims

- The parent's wall-clock hard timeout (`timeout_seconds + 20`) builds the same
  `TimedOutWithMetadata` payload as the other failure paths, but no test waits
  30 seconds to trigger it. Child-reported `timed_out` is covered by the
  non-completed stop-reason test.
- Ontology `SquadMember` nodes for failed spawns (CQ13) are DK-005, not done.
- The child reverse link is not yet projected into the ontology; DK-005.
- Nested children inherit the immediate parent run but not an ancestor session
  (the child runtime has no session id), so a grandchild's parent session is
  empty; this is honest absence, not a guessed id.
