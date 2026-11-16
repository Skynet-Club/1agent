# 0.99 GK-005 — `1agent goal status|link|close` evidence

Date: 2026-11-16
Scope: black-box CLI over the hash-chained goal log.
Result: **implemented and verified; GoalKernel is still not accepted as a
whole.**

All fixtures are synthetic and live in temporary directories. No production
journals, prompts, credentials or secrets are included.

## 1. Tested tree

| Field | Value |
|---|---|
| Repository | `/home/skynet/1-agent` |
| Branch | `0.99-dev` |
| Slice tag | `0.99-gk005-goal-cli` |
| Changed files | `src/main.rs`, `tests/goal_cli.rs`, `docs/architecture.md`, `docs/0.99-handoff.md` |

## 2. Commands and results

```bash
cargo test --offline --test goal_cli
cargo test --offline --lib
./scripts/verify.sh
```

| Command | Result |
|---|---|
| `cargo test --offline --test goal_cli` | **4 passed, 0 failed** |
| `cargo test --offline --lib` | **1113 passed, 0 failed, 4 ignored** |
| `./scripts/verify.sh` | **passed** (fmt, clippy all-targets `-D warnings`, all tests, core-v3 8/8) |

CLI tests:

- `goal_status_and_link_round_trip` — status lists goal/commitment/binding;
  link binds a session and a run; status shows both.
- `goal_close_requires_a_linked_run_and_passed_verification` — close fails
  without a linked run and with failed verification; succeeds with both;
  status reports achieved.
- `goal_close_abandoned_requires_listing_pending_commitments` — unlisted
  pending commitments fail; listing them cancels them and abandons the goal.
- `goal_link_requires_a_session_or_run` — neither target fails with a clear
  message.

## 3. Acceptance mapping

| GK-005 criterion | Evidence |
|---|---|
| `goal status` | JSON and human output; `--goal-id` filter; unknown goal fails |
| `goal link` | Session and run binding through GK-001/002 APIs |
| `goal close` | Achieved evidence gate and abandoned commitment cancellation through GK-003/004 |
| Negative exits | Four tests cover the main failure modes |
| No new storage | Reads/writes only the existing hash-chained goal log |

## 4. Explicit non-claims

- GoalKernel is not accepted; independent QA and the ontology Goal/Commitment
  nodes (GK-006) remain.
- `goal create` is not added; onboarding still creates the first goal.
- The TUI does not call `goal close` automatically; the CLI and library
  remain explicit surfaces.
