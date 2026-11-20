# MA-005 lease heartbeat and expiry reaper (2026-11-20)

Status: **landed; deterministic fake-worker tests and journal wiring pass**

## Problem

A Master lease had an expiry timestamp but no production heartbeat and no
automatic recovery. An expired lease still blocked a new lease, so a silent or
dead worker required a manual `master reclaim`. Nothing stopped an old worker's
late receipt from being considered after a replacement lease.

## Landed

### Heartbeat

- `TaskLease` now stores `duration_ms` and `last_heartbeat_unix_ms` with serde
  defaults, so old state JSON still loads.
- `MasterState::heartbeat(task, generation, now)` extends a live lease by its
  original duration and records the heartbeat time.
- A stale generation returns `StaleReceipt`; an already expired lease returns
  `LeaseExpired`. A dead lease cannot be revived by heartbeating.
- The heartbeat source is the supervisor's live observation: `master poll` and
  the TUI watch loop renew the lease only when they observe
  `Running`/`AwaitingInput`/`Blocked`. A worker does not have a separate
  heartbeat endpoint in this version.

### Expiry reaper

- `MasterState::expired_leases(now)` returns expired leases for tasks in
  `Assigned`, `Starting`, `Running`, `AwaitingInput` or `Blocked`, and returns
  none when the run is paused or rejected. An `Assigned` lease (the process
  crashed before start) is returned to `Retrying` without a kill.
- `MasterState::reap_expired_lease(task, generation, execution_terminated, now)`
  fails closed unless the old execution is confirmed terminated. The caller
  terminates the old Workshop session first; `master poll` and the TUI watch
  loop do this and log `termination failed` without retrying when termination
  cannot be confirmed.
- First two reaps move the task to `Retrying`; the third moves it to `Blocked`
  for human intervention. `lease_reaps` resets on a receipt, a human retry or a
  manual reclaim. The reaper never resurrects the old lease.
- Each successful reap records a `LeaseReaped` decision and journal event;
  `host_events` projects it as `task.queued`.
- `master ready` and `master start` reap expired leases before leasing new
  work; `master poll` observes/heartbeats first, then reaps what did not renew.

### Fencing

The generation remains the fencing token. After a reap, the old generation's
receipt is rejected with `StaleReceipt`; a replacement lease gets
`generation + 1` and its receipt is accepted only under the new generation.

## Tests

`cargo test --lib` — 1,332 passed / 0 failed / 4 ignored. `cargo test --bin
first-principles-agent` — 67 passed / 0 failed. MAST fault injection 7 passed.
`cargo clippy --tests` clean.

New deterministic tests:

- `heartbeat_extends_a_live_lease_and_rejects_stale_generations`: expiry moves
  from 100 to 190; stale generation is rejected; heartbeat at 190 fails as
  expired.
- `expired_lease_is_reaped_after_termination_and_old_receipt_is_fenced`: reap
  without termination fails; with termination goes to `Retrying`; the old
  receipt is `StaleReceipt` before and after a replacement lease; the new
  generation's receipt is accepted and resets `lease_reaps`.
- `repeated_lease_reaps_block_until_a_human_retry`: first reap `Retrying`,
  second `Blocked`, human retry resets the counter.
- `lease_reaper_skips_active_and_paused_runs`: active leases and paused runs are
  not reaped.
- `lease_expired_before_start_is_retried_without_killing_anything`: an
  `Assigned` lease that expired before `begin_starting` returns to `Retrying`.
- `expired_master_lease_is_reaped_and_journaled` (bin): real `MasterStore`
  journal round-trip; an untraceable Workshop prefix leaves the lease for human
  reclaim; a successful reap reloads as `Retrying` and the last event is
  `LeaseReaped`.

## Secondary fix found by the full bin gate

`master verify` returned as soon as `acceptance::close` was not `Accepted`.
For a missing criterion, `close` returns an error, so the task stayed
`ArtifactReady` instead of being persisted as `Rejected`. The verification path
now persists the rejection and appends a `Rejected` journal event, then still
returns an error to the CLI. The existing
`master_verify_rejects_false_success_missing_criteria_and_sha_drift` test now
passes.

## What this does not prove

- No live Workshop process was killed in this step. The failure path uses a
  nonexistent Workshop prefix; the successful path uses the existing fake
  Workshop fixture and the adapter's session-cancel contract.
- No multi-Master failover, clock-skew, or heartbeat-cadence test. The heartbeat
  cadence is the poll/watch invocation cadence, not a background timer.
- Automatic reap count is a local counter (`lease_reaps`); it is not a
  tree-level budget or a distributed consensus token.
- For an execution target with no session (or no termination channel), the
  reaper fences the old generation but cannot stop the old process from writing
  files; the caller is expected to terminate or leave the lease for human
  reclaim.

## Evidence

- `unit-tests.txt` — 20 coordination tests, including the new heartbeat/reaper
  cases.
- `wiring-test.txt` — real-store/journal reaper test.
- `acceptance-regression.txt` — existing verification rejection test.
- `metrics.json` — counts and policy.
- `sha256.txt` — source and evidence digests.
