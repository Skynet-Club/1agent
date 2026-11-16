# 0.99 AR-001/002 — ActionRecord and gate policy/approval evidence

Date: 2026-11-16
Scope: `ActionStatus`/`ActionRecord` embedded in the existing hash-chained
action log, and runtime gate writes for allow / deny / require-approval.
Result: **implemented and verified; AR-003 reconciliation, AR-004 approval SHA
binding and the `1agent action show|verify` CLI remain open.**

All fixtures are synthetic and live in temporary directories. No production
journals, prompts, credentials or secrets are included.

## 1. Tested tree

| Field | Value |
|---|---|
| Repository | `/home/skynet/1-agent` |
| Branch | `0.99-dev` |
| Slice tag | `0.99-ar002-action-record` |
| Changed files | `src/action_log.rs`, `src/runtime.rs`, `src/tools/mod.rs`, `src/execution_gate.rs`, `src/main.rs`, `docs/architecture.md`, `docs/0.99-handoff.md` |

## 2. Commands and results

```bash
cargo test --offline --lib
./scripts/verify.sh
```

| Command | Result |
|---|---|
| `cargo test --offline --lib` | **1097 passed, 0 failed, 4 ignored** |
| `./scripts/verify.sh` | **passed** (fmt, clippy all-targets `-D warnings`, all tests, core-v3 8/8) |

Focused tests added:

- `action_log::tests::action_records_embed_in_the_hash_chain_and_the_latest_wins`
  — `allowed` then `executed` for one action id; latest wins; tampering with the
  embedded payload breaks the chain.
- `action_log::tests::legacy_entries_without_action_payload_still_verify`
  — entries without the optional payload keep their original hash and remain
  readable.
- `runtime::tests::gated_allow_records_executed_action_and_policy_metadata`
  — allow path records `executed`, writes action id, policy outcome/reason,
  authority profile, risk and reversibility to observation metadata.
- `runtime::tests::gated_deny_records_the_denied_action` — deny path records
  `denied` and the `authority_denied` observation carries policy metadata.
- `runtime::tests::gated_approval_records_needs_approval_without_executing`
  — a write-capable tool at Write strength records `needs_approval`, the tool
  is never executed, and risk/reversibility/profile are recorded.

## 3. Acceptance mapping

| AR-001/002 criterion | Evidence |
|---|---|
| `ActionRecord` schema fields | Struct has action/run/session/type/risk/reversible/policy outcome+reason/profile/approval card/status/artifact SHA |
| Old action log readable | Optional payload with `serde(default, skip_serializing_if)`; legacy hash payload unchanged; dedicated test |
| Gate writes allow / deny / approval | Three runtime tests; the gate remains the only decision source and the runtime only records its result |
| Journal side carries policy/approval metadata | `action_id`, `policy_outcome`, `policy_reason`, `authority_profile_id`, `approval_card_id` (when present), `risk`, `reversible` on the `ToolObservation` |
| Fail closed before execution | A gated call whose pre-execution record cannot be persisted returns `action_record_error` and does not run the tool |

## 4. Explicit non-claims

- AR-003 (action log ↔ journal reconciliation) is not implemented. The same
  `action_id` and policy fields are written to both sides, but no verifier
  compares them yet.
- AR-004 (approval card bound to artifact SHA and re-approval on drift) is not
  implemented. `artifact_sha256` is copied from the tool's `sha256` metadata
  when present.
- The approval card id comes from tool-level approvals (e.g. browser cards);
  Authority-level `require_approval` records have no card id yet.
- Action records are only produced when a run has an execution gate. The TUI
  does not configure a gate; gated CLI runs write
  `<journal-dir>/action-records.jsonl`.
- `reversible` defaults to `!requires_write`; tools do not declare finer
  reversibility yet.
