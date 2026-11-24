# Multi-agent tails — 2026-11-24

Closes the three remaining Batch 3 items on `1.1-dev`. The Master slice of
MA-009 remains advisory; this change turns child-journal writes into a bounded
compensation plan but does not auto-delete.

## MA-007 — required capabilities/tools at lease time

`MasterState::lease_task` now re-checks the Work Item's
`required_capabilities` and `required_tools` against the leased
`AgentProfile`. A stale or forged assignment fails closed with
`MissingCapabilities` / `MissingTools` and leaves no lease.

Capability names match `AgentProfile.capabilities` directly; there is no
heuristic capability-to-tool translation.

Test: `master_coordination::tests::lease_enforces_required_capabilities_and_tools`.

## MA-008 — failed-child usage fallback

The child run prints usage totals in its JSON when spawned with
`ONEAGENT_SUBAGENT_REPORT_USAGE=1`. The parent still prefers the child journal;
if the journal is missing or unreadable it charges the JSON totals instead.
Ordinary `1agent run --json` output is unchanged, so strict consumers such as
the chat bridge keep their `RunResult` contract.

Tests:

- `tools::subagent::tests::uses_child_reported_usage_when_the_journal_is_unavailable`
- `chat_bridge_converts_messages_and_results` (strict JSON compatibility)

## MA-009 — bounded child compensation plan

The spawn result now includes:

```json
"compensation": {
  "automatic": [{"path": "...", "action": "remove", "sha256": "...", "reason": "created by the subagent"}],
  "manual": [{"path": "...", "action": "manual", "reason": "existing file modified; no pre-write content snapshot"}]
}
```

Observation metadata carries `compensation_automatic` / `compensation_manual`
counts. Files created by the child can be safely removed; overwritten or edited
files are manual because no pre-write content snapshot exists. Master does not
consume this field automatically yet.

Tests:

- `tools::subagent::tests::child_compensation_removes_created_files_and_flags_modified_files`
- `tools::subagent::tests::completed_child_with_valid_journal_preserves_structured_answer`

## Verification

- `scripts/verify.sh` green; lib 1459/0/4, bin 73/0, core eval 8/8.
- Focused `chat_bridge_cli` 4/4; subagent module 36/36.
- Installed `~/.local/bin/1agent` sha256 `f91a23b9…`; previous `b24040a7…`
  saved as `~/.local/bin/1agent-pre-multiagent-tails-2026-11-24`.
