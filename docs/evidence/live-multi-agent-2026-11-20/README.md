# MA-014 live multi-agent harness / fault injection (2026-11-20)

Status: **fault-injection negatives landed; live Ollama smoke ran but did not
produce a successful child collaboration**

## Deterministic fault injection

- `timed_out_spawn_is_killed_and_marked_timed_out`: a test-only 1-second
  timeout override makes the tool take the real timeout branch; the fake
  sleeping child is killed, the observation is `TimedOutWithMetadata`, and the
  registry record is `TimedOut`.
- `completed_child_with_audit_failure_is_rejected`: a completed child returns a
  journal with a non-sequential event (`seq 0` then `seq 5`); the parent rejects
  it with `audit failed`, and the registry record is `Failed`.
- `replaying_the_same_spawn_call_returns_the_cached_child` now also asserts the
  registry has exactly one record id for the workspace after the retry. The
  duplicate call returns the cached outcome and starts no second child.
- `failed_task_without_applied_effects_has_no_repair`: a Master task that fails
  before any artifact is verified gets `repair = None` and no
  `RepairPlanned` decision, so a clean failure does not invent compensation.

## Live smoke

Environment: local Ollama `qwen3-8b-16k:latest` (tools-capable), started
manually at `127.0.0.1:11434`, installed `~/.local/bin/1agent` (MA-013 build).
Task: use `spawn_agent` exactly twice; Child A lists three facts about 42,
Child B verifies them, then combine and report conflicts.

- Run 1 (`run.json`): parent `stop_reason=completed`; the parent journal has
  two `spawn_agent` calls and two observations. Child A failed with
  `subagent exited 1: no provider route for mode Execute`; Child B failed with
  `needs_authorization` because the read-only child was classified as an
  execution task. Both failures returned structured metadata; the child usage
  fields were present (all zero because no model response was recorded).
- Run 2 (`run2.json`): the parent itself was classified as Execute and exited 1
  with `no provider route for mode Execute`; no child was spawned.

The live run proves the parent's spawn path, structured failure handling and
usage metadata on a real local model. It does **not** prove a successful
two-child collaboration: the provider route has no Execute mode for these
child tasks, and the local 8B did not avoid that classification. Per the plan,
local model quality is not a release gate.

## Gates

Final full gate (captured in `gate.txt`, `fault-tests.txt`):

- lib 1,361 passed / 0 failed / 4 ignored;
- bin 68 passed;
- CLI e2e 39 passed with the one pre-existing failure skipped
  (`cli_tui_agents_refresh_uses_configured_profiles`; it fails on unmodified
  HEAD too with `session_limit：活动会话最多 128 个`);
- MAST fault injection 7 passed;
- eval `evals/core.json` 8/8 PASS;
- strict `cargo clippy --offline --all-targets -- -D warnings` clean;
- `cargo fmt --check` clean.

## Blocker during this slice

`agent_exec` could not create a new persistent PTY:
`create_failed: session_limit：活动会话最多 128 个` (skynet-term daemon at its
active-session limit). The remaining verification, evidence capture and commit
ran through the visible Bash tool; this is recorded rather than silently
replacing the PTY path.

## What this does not prove

- No successful live child answer or cross-process fanout result.
- The live failure is a provider-route/permission classification issue, not a
  spawn-budget, process-group or idempotency failure; those are covered by the
  deterministic suites.
- The test-only timeout override is not a production configuration; production
  still clamps child timeouts to 10..=300 seconds.
- The CLI e2e baseline failure is an environment session-limit issue in the
  test TUI, not caused by MA-014.

## Release provenance

Feature commit `75713e5`; release binary `5f93edb7…` installed as
`~/.local/bin/1agent`; previous MA-013 build `9277e48f…` backed up as
`1agent-pre-live-faults-2026-11-20`. Installed smoke: `1agent --version` and
`1agent run --help` exit 0. Details in `release.txt`; source and evidence
digests in `sha256.txt`.
