# MA-011 process group / registry reconcile (2026-11-20)

Status: **landed; root spawns own a process group, Linux children get a
parent-death signal, and the registry converges stale records**

## Scope

A cancelled or timed-out spawn must not leave a child or its descendants
running. A parent `SIGKILL` must not leave orphans. Registry records whose
process disappeared must stop showing as active.

## Landed

- A root spawn (`depth == 0`) starts the child in its own process group
  (`process_group(0)`). A deeper spawn inherits its ancestor's group, so a root
  group kill covers the whole depth-2 subtree. Depth 2 is the leaf.
- On Linux/Android the child sets `PR_SET_PDEATHSIG = SIGKILL` in `pre_exec`
  and re-checks `getppid` before exec. If the parent dies, the direct child is
  killed; the same setting on every spawn cascades down the tree. Other Unix
  targets skip this kernel feature.
- Cancellation, timeout, wait failure and normal child exit all call
  `kill_spawn_tree`: at depth 0 this sends `SIGKILL` to the child's process
  group, cleaning up any descendants the child left behind. Deeper spawns rely
  on `kill_on_drop` for the direct leaf.
- `subagent_registry::reconcile_stale_records` marks an active record `Failed`
  when its child pid (or, for `Starting`, its parent pid) is gone, with a
  2-second grace period to avoid racing a parent that is still recording a
  just-exited child. `load_recent_current` reconciles before returning records,
  so the TUI converges on load and refresh.
- `pid_alive` is shared between the registry and the tree budget.
- The idempotency ledger now calls `unlock` explicitly in `Drop`. A forked
  child can briefly share the open file description; closing the parent's fd
  alone did not always release the `flock` in the parallel test suite.

## Tests

- `root_spawn_starts_a_new_process_group`: the fake child's `ps` pgid equals the
  registry's `child_pid`.
- `deeper_spawn_stays_in_its_ancestor_process_group`: a depth-1 spawn's pgid
  equals the test process's pgid.
- `cancelling_a_root_spawn_kills_its_descendants`: the fake child starts a
  `sleep 60` grandchild, the spawn is cancelled, and the grandchild is dead
  within the poll window.
- `stale_records_converge_while_live_and_fresh_records_are_kept`: a dead child,
  a dead starting parent and a live child are distinguished; a fresh record is
  left alone.
- Ten consecutive `cargo test --offline --lib` runs passed after the explicit
  unlock fix.

Full gates (`gate.txt`, `eval.txt`):

- lib 1,356 passed / 0 failed / 4 ignored;
- bin 67 passed;
- CLI e2e 39 passed with the one pre-existing baseline failure skipped
  (`cli_tui_agents_refresh_uses_configured_profiles`, also fails on unmodified
  HEAD `264fa34`);
- MAST fault injection 7 passed;
- eval `evals/core.json` all scenarios PASS;
- strict `cargo clippy --offline --all-targets -- -D warnings` clean;
- `cargo fmt --check` clean.

## What this does not prove

- `PR_SET_PDEATHSIG` is Linux/Android only and is not exercised by a test that
  kills the test process. The cancellation path and group kill are tested.
- This is process-group cleanup, not a sandbox. A child with write/execute
  permission can call `setsid` or start a new session and escape the group; a
  seccomp/namespace sandbox is out of scope.
- Root group kill assumes depth 2 remains the leaf. If `MAX_DEPTH` grows,
  deeper spawns need their own grouping or a tree walk.
- Registry reconciliation runs when the UI loads current records, not on every
  CLI invocation. A reused pid can delay convergence.
- The 2-second grace period means a dead child can briefly still show as
  running after the UI reads the registry.

## Release provenance

Feature commit `a992b44`; release binary `811df54a…` installed as
`~/.local/bin/1agent`; previous MA-010 build `72619963…` backed up as
`1agent-pre-process-group-2026-11-20`. Installed smoke: `1agent --version` and
`1agent run --help` exit 0. Details in `release.txt`; source and evidence
digests in `sha256.txt`.
