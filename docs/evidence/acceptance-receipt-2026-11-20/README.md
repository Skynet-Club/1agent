# MA-003 acceptance receipt binding (2026-11-20)

Status: **wired and passing the CLI master verification e2e path**

## What changed

`master verify` no longer stops at “criteria look okay”. It now:

1. Builds an `AcceptanceContract` from the Work Item (objective, acceptance
   criteria, expected artifacts, risk).
2. Builds a `verification::VerificationInput` with:
   - executor = the assigned Agent,
   - verifier = the submitted `verified_by`,
   - criteria results from the submission,
   - **artifact checks recomputed by the runtime** via `inspect_artifacts`
     (the submission cannot supply artifact checks),
   - no hidden criteria (the Work Item schema has none yet).
3. Calls `acceptance::close`, which runs `verification::verify` and returns a
   `CompletionReceipt`.
4. Requires `receipt.decision == Accepted`; `Conditional`, `Rejected` and
   `Hacked` all fail before the task can become `verified`.
5. Stores the receipt in `VerificationResult.acceptance_receipt` and includes
   the contract digest in the journal reason.

`acceptance::close` also enforces executor ≠ verifier; the CLI keeps its own
explicit check. `verifier:`/`agent:` submissions map to `Independent`,
`human:` maps to `Human`, anything else maps to `Automated`.

## Verification

- `cargo test --lib`: **1320 passed / 0 failed / 4 ignored**.
- `cargo test --test cli_e2e cli_master_runs_verifies_and_replays_a_workshop_work_item`: pass.
  The test asserts:
  - `verification.status == "verified"`,
  - `verification.acceptance_receipt.decision == "accepted"`,
  - a non-empty `contract_digest`.
- `cargo test --test cli_e2e cli_tui_master_verification_commands_require_independent_artifact_evidence`: pass.
- `acceptance` unit tests already cover fake SHA, self-verification and
  conditional/hacked decisions; the CLI now routes through them.
- `cargo clippy --lib`: no warnings.

Installed binary: `~/.local/bin/1agent` sha256 `550ca10a…`; previous
`3781e67b…` backed up as `1agent-pre-acceptance-receipt-2026-11-20`.

## What this proves

- A Work Item cannot become `verified` without an accepted independent receipt.
- Artifact existence/SHA are recomputed from the workspace, not trusted from
  the verifier's JSON.
- Self-verification is rejected before acceptance.
- The receipt is durable in the Master state and journal-auditable.

## What this does not prove

- MA-005 lease heartbeat/reaper and MA-006 progress ledger/stall detection are
  still open.
- Hidden criteria and residual risks are not populated yet (empty lists).
- The verifier is whatever `master verify --input` supplies; there is no
  automatic verifier-agent selection or different-model enforcement yet.
- The e2e path is one Work Item; multi-Work Item goal acceptance with receipts
  is not separately tested here.

## Next

1. MA-006 Task/Progress Ledger + stall counter (Magentic-One pattern).
2. MA-005 lease heartbeat/reaper/fencing.
3. MAST fault-injection cases for fake SHA, self-verification, stall and
   duplicate actions.
