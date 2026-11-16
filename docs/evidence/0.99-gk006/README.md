# 0.99 GK-006 — ontology Goal/Commitment projection evidence

Date: 2026-11-16
Scope: read-only export of the hash-chained goal log as Goal/Commitment
nodes, including bindings.
Result: **implemented and verified; GoalKernel acceptance still requires
independent QA.**

All fixtures are synthetic and live in temporary directories. No production
journals, prompts, credentials or secrets are included.

## 1. Tested tree

| Field | Value |
|---|---|
| Repository | `/home/skynet/1-agent` |
| Branch | `0.99-dev` |
| Slice tag | `0.99-gk006-goal-ontology` |
| Changed files | `src/ontology/export.rs`, `src/ontology/mod.rs`, `src/ontology/validate.rs`, `src/main.rs`, `tests/ontology_cli.rs`, `docs/architecture.md`, `docs/0.99-handoff.md` |

## 2. Commands and results

```bash
cargo test --offline --lib
cargo test --offline --test ontology_cli
./scripts/verify.sh
```

| Command | Result |
|---|---|
| `cargo test --offline --lib` | **1114 passed, 0 failed, 4 ignored** |
| `cargo test --offline --test ontology_cli` | **26 passed, 0 failed** |
| `./scripts/verify.sh` | **passed** (fmt, clippy all-targets `-D warnings`, all tests, core-v3 8/8) |

Focused tests added:

- `ontology::export::tests::goal_log_exports_goals_commitments_and_bindings_without_prose`
  — active/achieved/kept statuses, commitment ownership, session stub, run
  ids, SHA+chars privacy, deterministic SHA and validator acceptance.
- `tests/ontology_cli.rs::goal_journal_export_is_deterministic_and_validates`
  — `ontology export/validate --goal-journal` black-box; repeated export bytes
  are identical.

Validator additions: Goal status accepts active/paused/achieved/abandoned;
Commitment requires a known status; `hasCommitment` and `belongsToGoal` are
closed references.

## 3. Acceptance mapping

| GK-006 criterion | Evidence |
|---|---|
| Goal/Commitment nodes with status/timestamps | Unit and CLI tests |
| Title/description privacy | SHA + char count only; raw prose absent from the export |
| Bindings | `partOfSession` session stub and literal `runIds` |
| No dangling references | `hasCommitment`/`belongsToGoal` validated closed-world |
| Deterministic | Same log exports identical bytes/SHA |
| Old goal log readable | Replay via `GoalGraph::replay`; closed/terminal statuses covered |
| Read-only | The exporter reads the file directly; no GoalStore lock/repair |

## 4. Explicit non-claims

- GoalKernel is not accepted as a whole; G1/G4 need independent QA and the
  remaining 0.98.2 quality runs.
- The goal-log export does not merge with a session export; it emits a
  separate document with a Session stub.
- Run ids in `runIds` are literals, not references, because the runs' journals
  may not be in the same document.
- DK-006 (TUI squad lineage summary) remains open.
