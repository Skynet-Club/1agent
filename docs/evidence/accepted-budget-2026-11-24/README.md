# Accepted receipt survives an exhausted final-answer attempt — 2026-11-24

The post-persist eval found one `checksum-file` miss where the model obtained
an accepted `acceptance_gate` receipt, but its final tool-free answer attempt
failed and the step-budget path returned `needs_evidence` with 0 claims and 0
evidence. The delivery was verified; only the narration was missing.

## Fix

At step-budget finalization, if the completion gate has an **accepted**
receipt and the budget answer is `needs_evidence`, the runtime now synthesizes
`accepted_budget_answer`:

- status `completed`;
- conclusion names each verified artifact with its sha256, the acceptance task
  id and the stop reason;
- claims/evidence are backfilled from the verified artifacts or the accepted
  receipt;
- one flip condition: the accepted artifact was modified after delivery;
- the generic high-impact "budget verification" unknown is dropped because the
  acceptance contract already verified the delivery.

The normal model final answer still wins when it exists; this path only
replaces a `needs_evidence` budget answer when an accepted receipt is present.

## Tests

- `runtime::tests::accepted_receipt_survives_exhausted_final_answer_budget`:
  write → verify + accepted gate at the step budget, with the final provider
  attempt returning an invalid turn. Asserts `completed`, stop reason
  `completed`, claims/evidence/flip present, no unknowns, and the sha256 in the
  conclusion.
- `verify.sh` is green; lib **1454/0/4**, bin **73/0**, core eval 8/8.

## Real eval

Checksum-file only, N=3, binary `25be45d2…`:

- **3/3 passed, pass^3 100%**, no issue samples
  (`docs/evidence/accepted-budget-2026-11-24/report.json`).

The full 24-case eval before this fix was 22/24; the remaining
`research-compare` miss (completed instead of conditional) is a separate
answer-generation issue, not this budget path.

## Binary

Final installed `~/.local/bin/1agent` sha256 `25be45d2c387299eadcabc0a…`;
previous `2af507a9…` saved as
`~/.local/bin/1agent-pre-budget-accept-2026-11-24`.
