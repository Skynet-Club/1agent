# 1AO P1 — action tool filtering by capability contract (2026-11-19)

Scope: close the last P1 gate item. Action tools generated from
`ontology/actions.json` are now visible and executable only when the capability
preflight selects their manifest for the task.

## Behavior

- `capabilities/manifests.json` gains a `change-proposal/v1` hint for
  `propose a change` / `change proposal` / `修改提案` / `变更提案`. Its
  requirements select `code.propose_change`, `code.run_checks` and
  `code.prepare_release`.
- The runtime filters action `ToolSpec`s and the advertised capability snapshot
  to the selected action tools. Other action tools stay hidden.
- `request_tools` denies hidden action tools with `capability_not_selected`.
- A direct call to a hidden action tool is denied by the runtime executor with
  `capability_not_selected` before Authority/Policy evaluation.
- A run without a matching contract sees no action tools at all.

Non-action tools are unaffected. The registry still contains every tool, so
`1agent tools` and direct CLI/library use keep working.

## Verification

| Check | Result |
|---|---|
| `cargo test --offline --lib capability_preflight_controls_action_tool_visibility_and_execution` | 1 passed |
| `cargo test --offline --lib` | 1221 passed, 0 failed, 4 ignored |
| `./scripts/verify.sh` | **1676 passed, 0 failed, 13 ignored** |
| claims | 267 (243 verified / 16 partial / 8 no evidence) |

Raw: `verify-p1-filter.log`.

## Limits

- Selection is keyword-based through the existing deliverable hints, not a
  semantic planner.
- The capability hint selects the whole code-release chain; finer per-action
  selection needs an explicit task contract field in P2.
- Hidden-tool denial happens in the runtime path; direct `ToolRegistry` use
  outside a run is unchanged.
- TUI approval queue remains unbuilt; approvals are CLI-only.
