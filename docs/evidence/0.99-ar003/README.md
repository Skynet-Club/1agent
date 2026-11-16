# 0.99 AR-003 — action log ↔ journal reconciliation evidence

Date: 2026-11-16
Scope: read-only reconciliation between a run journal's gated
`ToolObserved` entries and the latest `ActionRecord`s.
Result: **implemented and verified; AR-004 approval SHA binding and AR-006 CLI
remain open.**

All fixtures are synthetic and live in temporary directories. No production
journals, prompts, credentials or secrets are included.

## 1. Tested tree

| Field | Value |
|---|---|
| Repository | `/home/skynet/1-agent` |
| Branch | `0.99-dev` |
| Slice tag | `0.99-ar003-reconcile` |
| Changed files | `src/action_reconcile.rs`, `src/lib.rs`, `src/runtime.rs`, `docs/architecture.md`, `docs/0.99-handoff.md` |

## 2. Commands and results

```bash
cargo test --offline --lib
./scripts/verify.sh
```

| Command | Result |
|---|---|
| `cargo test --offline --lib` | **1104 passed, 0 failed, 4 ignored** |
| `./scripts/verify.sh` | **passed** (fmt, clippy all-targets `-D warnings`, all tests, core-v3 8/8) |

Focused tests added:

- `action_reconcile::tests::matching_journal_and_action_records_reconcile`
- `action_reconcile::tests::a_missing_action_record_is_reported`
- `action_reconcile::tests::a_missing_journal_observation_is_reported`
- `action_reconcile::tests::status_policy_and_artifact_mismatches_are_reported`
- `action_reconcile::tests::records_from_other_runs_are_ignored`
- `action_reconcile::tests::non_gated_journals_reconcile_without_actions`
- `runtime::tests::gated_run_journal_and_action_log_reconcile` — a real gated
  scripted run, then the journal and action log are read back and reconcile.

## 3. Acceptance mapping

| AR-003 criterion | Evidence |
|---|---|
| One run's journal and action log correspond one-to-one | End-to-end runtime test reconciles a real run |
| Missing on either side is an error | Missing record and missing observation tests |
| Non-gated runs are not misjudged | Observation without `action_id` and no records → `checked=0`, `ok` |
| Issues name the action id and reason | `ActionReconcileIssue { action_id, kind, detail }` |
| No writes or repair | The module only borrows events and records, returning a report |

## 4. Explicit non-claims

- There is no automatic enforcement at run end; the caller decides whether to
  fail. `1agent action verify` (AR-006) is not implemented.
- An empty journal returns `ok` with `checked=0`; with no run id the reconciler
  cannot attribute records, so callers must pass the run's records.
- AR-004 approval-card SHA binding is not implemented.
- Reconciliation does not validate the action log hash chain itself;
  `ActionLogJournal::open` already verifies it.
