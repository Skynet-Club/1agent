# MA-010 spawn idempotency (2026-11-20)

Status: **landed; completed spawns replay by tool-call id**

## Scope

A spawn call is identified by `(parent_run_id, tool_call_id)`. Replaying the
same call returns the cached completed result instead of starting a second
child. A per-call exclusive lock rejects a concurrent duplicate attempt.

## Landed

- `src/tools/spawn_ledger.rs` layers spawn semantics on the existing
  `IdempotencyStore`:
  - one small JSONL store per key under
    `~/.cache/1agent-tmp/subagents/spawn-idempotency/<sha256>.jsonl`;
  - an exclusive `.lock` file held for the duration of the attempt;
  - `try_lock` returns `spawn already in progress for this call id` when the
    same key is already executing (in-process or cross-process);
  - `record` stores the complete `ToolOutcome`; `replay` deserializes it.
- `spawn_agent` computes a request fingerprint over the task, workspace
  argument, step/timeout limits, intersected write/network permissions, read
  roots and allowed tools. Reusing a call id with different arguments hits the
  store's conflict path and fails closed before any child starts.
- A replay returns the original `ToolOutcome` with `idempotent_replay=true` in
  metadata. It does not create a workspace, reserve a tree slot, charge usage
  again or write a second registry record.
- The replay check runs before the fanout counter and workspace resolution, so a
  retried call does not consume fanout budget or create an unused auto
  workspace. Failed/timed-out/cancelled attempts are not recorded; a later
  retry may start a new child.
- This is the first consumer of `src/idempotency.rs` outside the CLI; the store
  format, fingerprint conflict and partial-line handling are reused as-is.

## Tests

- `replaying_the_same_spawn_call_returns_the_cached_child`: first call runs the
  fake child once; the second call with the same run id/call id returns the
  cached outcome (`idempotent_replay=true`, same `child_run_id`) and the fake
  program's invocation count stays 1. A different task with the same key fails
  with `spawn idempotency`; a new run id is a new key and spawns again.
- `replay_returns_the_recorded_outcome_after_a_restart`: record, drop, reopen,
  replay; a different fingerprint conflicts.
- `a_second_in_flight_handle_is_rejected`: the per-call lock returns
  `already in progress`.
- `fingerprints_are_stable_and_request_sensitive`.

Full gates (`gate.txt`, `eval.txt`):

- lib 1,352 passed / 0 failed / 4 ignored;
- bin 67 passed;
- CLI e2e 39 passed with the one pre-existing baseline failure skipped
  (`cli_tui_agents_refresh_uses_configured_profiles`, also fails on unmodified
  HEAD `264fa34`);
- MAST fault injection 7 passed;
- eval `evals/core.json` all scenarios PASS;
- strict `cargo clippy --offline --all-targets -- -D warnings` clean;
- `cargo fmt --check` clean.

## What this does not prove

- No live-model spawn test; fake child programs prove the replay mechanism.
- The cache has no TTL or size cap. Each completed call leaves one small JSONL
  file under the cache directory; cleanup is left to the cache owner.
- A crash after the child completes but before `record` can still duplicate on
  a retry. Closing that needs a two-phase completion record.
- Failed/timed-out/cancelled attempts are deliberately not cached; retrying the
  same call may spawn again.
- Spawns outside a `RunContext` (no parent run id) are not deduplicated.
- The lock is advisory and local-filesystem based; a network filesystem with
  unreliable `flock` semantics would weaken the guarantee.

## Release provenance

Feature commit `e6c27b0`; release binary `72619963…` installed as
`~/.local/bin/1agent`; previous MA-009 build `330f68e6…` backed up as
`1agent-pre-spawn-idempotency-2026-11-20`. Installed smoke: `1agent --version`
and `1agent run --help` exit 0. Details in `release.txt`; source and evidence
digests in `sha256.txt`.
