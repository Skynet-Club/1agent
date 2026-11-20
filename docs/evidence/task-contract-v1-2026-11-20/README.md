# TaskContract v1 live run (2026-11-20)

Status: **passed for DeepSeek and local qwen3-8b-16k; contract coverage is
enforced for every Work Item**

## What changed

`PlanProposal` now carries an optional `task_contract`; the CLI model generator
requires it. The contract is the task-understanding/decomposition source:

```json
{
  "objective": "one-sentence goal",
  "deliverables": [{
    "id": "safe-id",
    "statement": "observable deliverable",
    "acceptance": ["measurable criterion"],
    "work_item_ids": ["work-item-id"]
  }],
  "constraints": ["..."],
  "non_goals": ["..."],
  "sources": ["URL or local path read"],
  "acceptance": ["goal-level criterion"]
}
```

Validation:

- objective non-empty, bounded, control-free;
- 1..=8 deliverables, unique safe ids;
- every deliverable has a statement, at least one acceptance criterion and at
  least one Work Item;
- every Work Item belongs to exactly one deliverable;
- unknown/duplicate work-item references fail closed;
- dependencies still form a DAG, Work Items still need capability, criteria,
  artifacts and workspace confinement.

No model-name branch: the same prompt/parser/validator applies to all providers.

## Live results (installed binary `3781e67b…`)

| Run | Requests / calls | Elapsed | Contract |
|---|---:|---:|---|
| DeepSeek + Feishu | 6 / 5 | 52.7 s | 3 deliverables → 3 Work Items |
| Local qwen3-8b-16k + Feishu | 2 / 1 | 93.1 s | 3 deliverables → 4 Work Items (one deliverable covers two) |

Both plans are `approved=false`. The local model produced the contract in the
same response as the plan after one `read_source` call.

## What this proves

- A model-produced contract is feasible for a strong and a weak local model.
- Coverage is enforced: missing, unknown, duplicate or unassigned work-item
  references fail validation.
- The contract is optional at the store level (old plans remain readable), but
  the CLI planner requires it for new plans.

## What this does not prove

- Contract quality (are the deliverables the right ones?) is not measured; the
  validator checks coverage, not semantics.
- One run per model.
- Acceptance receipt binding is not implemented yet; it is the next slice
  (MA-003).
- The TUI production planner uses `GoalContract`/`KeyResult`; this contract is
  for the CLI `master plan` path.

## Tests

- `cargo test --lib`: **1320 passed / 0 failed / 4 ignored**.
- `master`: 6 tests, including contract coverage/duplicate/unknown/unassigned.
- `master_planner`: 24 tests, including a missing-contract rejection.
- `cargo clippy --lib`: no warnings.

## Evidence files

- `deepseek-contract-summary.json`, `deepseek-*.sha256`
- `ollama-contract-summary.json`, `ollama-*.sha256`
- `tool-traces.txt`

Full plans and journals stay local; only summaries and hashes are committed.
