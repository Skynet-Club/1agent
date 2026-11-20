# MA-008 tree-wide subagent budget (2026-11-20)

Status: **landed; deterministic tests pass; child journal usage is returned and
charged to the tree**

## Scope

One spawn tree shares a file-backed budget for concurrency, cost and tokens.
The budget file is inherited by every child process, so a nested spawn sees the
same counters instead of a fresh per-process limit.

## Landed

- `src/tools/subagent_budget.rs` adds `TreeBudget`: a JSON state file guarded by
  an exclusive file lock (`tree.json` + `tree.lock`).
- The root creates the file on the first spawn. Limits are fixed in the file:
  - `ONEAGENT_SUBAGENT_TREE_MAX_AGENTS` (default 16 concurrent children);
  - `ONEAGENT_SUBAGENT_TREE_MAX_COST_MICROS`;
  - `ONEAGENT_SUBAGENT_TREE_MAX_TOKENS`.
  Children inherit `ONEAGENT_SUBAGENT_TREE_FILE`; changing the limit env vars in
  a child cannot widen the file's limits.
- `spawn_agent` reserves one slot before starting the process and is rejected
  with `tree budget: ...` when concurrency or accumulated cost/token limits are
  reached. The slot is released on every exit path (drop or `finish`).
- Reservations record owner and child PIDs; dead reservations are pruned
  (`kill(pid, 0)` on Unix) so a crashed parent does not permanently wedge the
  tree.
- After the child exits, the tool reads its journal once, summarizes
  `ModelResponded` usage and charges input tokens, output tokens and cost to the
  tree. The same events feed the completed-child audit.
- The tool result gains `usage`:
  `{model_calls, input_tokens, output_tokens, cost_micros}`; the observation
  metadata carries `usage_*` keys. The failure path for a non-completed child
  also carries whatever usage its journal exposed.
- `spawn_agent`'s parameters are unchanged; limits are operator configuration,
  not a model-settable argument.

## Tests

Focused tests (`tree-budget-tests.txt`, 28 passed):

- `tree_budget_rejects_spawn_when_concurrency_is_full`: one live reservation,
  max 1 -> the next spawn fails before starting a child.
- `tree_budget_rejects_spawn_after_cost_limit`: finished child usage reaches the
  cost limit -> the next spawn fails.
- `completed_child_with_valid_journal_preserves_structured_answer`: child
  journal usage 100 in / 50 out / 7 micros is returned in the tool result and
  persisted as `spent_cost_micros=7`, `spent_tokens=150`, `active=[]`.
- `child_receives_parent_lineage_environment`: the child receives the tree file
  path alongside parent run/session/spawn-call ids.
- `concurrency_slot_is_released_on_drop`: dropping an unfinished reservation
  releases the slot.
- `finished_usage_is_charged_to_the_tree`: totals and empty active set persist.
- `cost_and_token_limits_reject_new_spawns`.
- `token_limit_rejects_new_spawns`.
- `limits_are_frozen_when_the_root_creates_the_file`: a second handle with
  wider limits still enforces the file's original max.
- `dead_reservations_are_pruned`: a reservation owned by a dead PID does not
  block a new spawn.
- `invalid_limit_text_is_rejected`.

Full gates (`gate.txt`, `eval.txt`):

- lib 1,347 passed / 0 failed / 4 ignored;
- bin 67 passed;
- MAST fault injection 7 passed;
- eval `evals/core.json` all scenarios PASS;
- strict `cargo clippy --offline --all-targets -- -D warnings` clean;
- `cargo fmt --check` clean.
- CLI e2e: 39 passed, 1 failed
  (`cli_tui_agents_refresh_uses_configured_profiles`). The same test fails on
  unmodified HEAD `264fa34`; it is a pre-existing failure, not caused by
  MA-008.

## What this does not prove

- No live-model spawn test. Fake child programs prove the mechanism; model
  quality is out of scope.
- A child that dies before its journal is readable is not charged; the tree can
  undercount that spend.
- The budget file is an overspend guard for cooperating agents, not a sandbox
  boundary. A child with write access can edit its own user-owned cache files.
- `1agent run --max-cost-micros` / `--max-tokens` still bound only that run.
  Tree-wide cost/token limits are configured with the `ONEAGENT_SUBAGENT_TREE_*`
  environment variables.
- Orphaned child processes are MA-011, not handled here.

## Release provenance

Feature commit `ebdef98`; release binary `951650c4…` installed as
`~/.local/bin/1agent`; previous MA-007 build `4f96acbe…` backed up as
`1agent-pre-tree-budget-2026-11-20`. Installed smoke: `1agent --version` and
`1agent run --help` exit 0. Details in `release.txt`; source and evidence
digests in `sha256.txt`.
