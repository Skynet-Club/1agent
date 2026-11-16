# 0.99 GK-001/002 — GoalBinding and session/goal/run link evidence

Date: 2026-11-16
Scope: `GoalBinding` projection in the existing hash-chained goal log, explicit
`SessionMission.goal_id`, and store-level session/run link helpers.
Result: **implemented and verified; GK-003 (close with evidence) remains open.**
This is not a GoalKernel acceptance and not a release.

All fixtures are synthetic and live in temporary directories. No production
journals, prompts, credentials or secrets are included.

## 1. Tested tree

| Field | Value |
|---|---|
| Repository | `/home/skynet/1-agent` |
| Branch | `0.99-dev` |
| Slice tag | `0.99-gk002-goal-binding` |
| Working tree | code + docs committed; only untracked 0.98.2 eval raw artifacts remain |
| Changed files | `src/goal.rs`, `src/tui_session.rs`, `src/ontology/export.rs`, `src/surface_service.rs`, `docs/architecture.md`, `docs/0.99-handoff.md` |

## 2. Commands and results

```bash
cargo test --offline --lib
cargo test --offline --test ontology_cli
./scripts/verify.sh
```

| Command | Result |
|---|---|
| `cargo test --offline --lib` | **1088 passed, 0 failed, 4 ignored** |
| `cargo test --offline --test ontology_cli` | **25 passed, 0 failed** |
| `./scripts/verify.sh` | **passed** (fmt, clippy all-targets `-D warnings`, all tests, core-v3 8/8) |

Focused tests added:

- `goal::tests::goal_binding_tracks_session_runs_and_commitments` — one goal,
  one session, two runs, two commitments; queries by goal/session/run;
  replay-snapshot equality.
- `goal::tests::binding_negative_cases_fail_closed` — unknown goal, empty
  ids, second active goal on one session, one goal on two sessions, one run on
  two goals, closed-goal linking, idempotent rebinding, and session reuse
  after the previous goal closes.
- `goal::tests::store_level_binding_helpers_persist_and_replay` —
  `bind_session_goal` / `link_run_to_goal` persist through `GoalStore` and
  survive reopen.
- `goal::tests::legacy_goal_snapshot_without_bindings_is_readable` — old
  snapshot JSON without `bindings` deserializes.
- `tui_session::tests::mission_goal_id_is_stable_across_revisions_and_reopen`
  — revision does not change the goal id; reopen preserves it.
- `tui_session::tests::legacy_session_mission_without_goal_id_normalizes_to_mission_id`
  — old `session.json` without `mission.goal_id` loads with the mission id.
- Existing goal-log replay, store tamper, and ontology integration tests still
  pass; the ontology Goal IRI now reads `mission.goal_id()` and stays stable
  because the id defaults to the mission id.

## 3. Acceptance mapping

| GK-001/002 criterion | Evidence |
|---|---|
| `GoalBinding` schema + migration | New struct/events, `GoalSnapshot.bindings` with `serde(default)`, legacy snapshot test, replay of logs without binding events |
| explicit session/goal link | `SessionMission.goal_id`, stable across revisions, legacy normalization |
| one session, several runs attributed correctly | Two runs linked to one goal and queried through `binding_for_run` |
| old goal logs readable | Existing replay tests plus the legacy snapshot/event compatibility above |

## 4. Explicit non-claims

- No TUI auto-linking yet: nothing in the TUI opens a `GoalStore` and calls
  `link_run_to_goal` on run completion. `1agent goal link/close` is GK-005.
- `GoalBinding` is derived from the goal log, not a second store; the log
  remains the audit source.
- `close_goal_with_evidence` and the "no evidence means paused/unknown" gate
  are GK-003 and are not implemented here.
- Ontology export still emits the existing Goal node; richer Goal/Commitment
  nodes are GK-006.
