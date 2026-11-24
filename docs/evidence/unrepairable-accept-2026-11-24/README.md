# Accepted delivery survives a lost final answer — 2026-11-24

The post-budget full eval found cases where a verified delivery with an
**accepted** gate receipt still ended `needs_evidence` because the final
narrative was missing: one at the step budget and two at failed answer repair.
The step-budget synthesis from `checkpoint-accepted-budget-2026-11-24` covered
only the first exit.

## Fix

`accepted_budget_answer` is now `accepted_delivery_answer` and is used at three
exits:

1. step-budget finalization (existing);
2. tool-call budget (new);
3. failed answer repair, for both the structured validation path and the
   plain-text fallback (new).

When the completion gate has an accepted receipt, the synthesized answer:

- has status `completed` and stop reason `completed`;
- names each verified artifact with its sha256, the acceptance task id, the
  stop reason and the acceptance criteria;
- backfills claims/evidence from verified artifacts, read sources named in the
  conclusion, or the accepted receipt;
- carries one flip condition targeting the backfilled delivery claim, so
  `AgentAnswer::validate` passes.

The invalid model answer stays in the journal and is not presented as verified.

A first full eval on intermediate binary `beec0467…` caught a missing piece:
the synthesized flip condition had no target, and `checksum-file` failed the
audit with "flip condition must reference at least one claim or unknown". The
final build backfills first and then targets the delivery claim.

## Tests

- `accepted_receipt_survives_exhausted_final_answer_budget`
- `accepted_receipt_survives_unrepairable_plain_text_final`
- `accepted_receipt_survives_unrepairable_json_final`

All assert `completed` + stop reason, claims/evidence/flip present, non-empty
flip targets, and `AgentAnswer::validate` passing. `scripts/verify.sh` is green;
lib **1455/0/4**, bin **73/0**, core eval 8/8.

## Final full eval

Binary `~/.local/bin/1agent` sha256 `7d338a8e…`, suite
`evals/real-tasks-v1.json`, N=3, `deepseek-flash` via `127.0.0.1:8787`:

- **23/24 attempts (95.83%)**, **pass^3 7/8 (8750 bps)**, at least once **8/8**.
- Failure class: `generation` 1.
- Per case: bug-fix 2/3, config 3/3, data-processing 3/3, research-compare 3/3,
  long-task 3/3, broken-json 3/3, checksum-file 3/3, directory-summary 3/3.
- The only miss is `bug-fix-last-index` repeat-3: expected `completed`, got
  `conditional`. The model stayed conditional after `verify_artifact` reported a
  media-type mismatch (`application/octet-stream` vs `text/rust`) even though the
  hash and content checks passed. This is an answer-generation / verification
  media-type issue, not an accepted-receipt loss.
- The pre-fix "before" run (binary `25be45d2…`) was 21/24:
  `docs/evidence/post-budget-fix-eval-2026-11-24/`.

## Evidence

- `report.json` — full eval report.
- `stderr.log` — empty.
- `product-eval/` — per-repeat `cases.jsonl` and journals; repeat directories
  are ignored by Git and kept locally for audit.

## Binary

Final installed `~/.local/bin/1agent` sha256 `7d338a8e…`; previous
`25be45d2…` saved as `~/.local/bin/1agent-pre-unrepairable-accept-2026-11-24`.
