# Answer repair, fast product eval and delivery view

Status: complete. Full product eval **22/24**, pass^3 **7/8**, pass at least
once 8/8. Fast product eval and final gates in the sections below.

## Answer repair

- `src/provider.rs`: the final reply parser accepts the first JSON object and
  ignores trailing prose or stray braces (`serde_json::Deserializer`), so a
  `...}}` tail no longer forces repair.
- `src/runtime.rs`: `normalize_evidence_links` drops an evidence → claim link
  when the claim does not declare that source, instead of rejecting the whole
  answer. The model response stays in the journal.
- `src/runtime.rs`: a completed delivery with side effects gets at least one
  verified claim and matching evidence backfilled from `verify_artifact`
  results or the accepted receipt; the plain-text fallback now emits claims.
- Prompt: `Claims/evidence must be non-empty` for completed deliveries.

### Iteration 1

`product-eval-iteration1/` is the run before link normalization: 19/24
(79.16%), pass^3 4/8, pass at least once 8/8. All three remaining
`needs_evidence` failures were the same pattern: a valid final answer arriving
on the budget turn with one evidence record pointing at a claim that did not
declare its source.

- bug-fix-last-index 1/3, data-processing-sales 2/3, checksum-file 2/3;
- config-retries 3/3, long-task-plan 2/3, research-compare, broken-json and
  directory-summary 3/3.

### Iteration 2

`product-eval-normalization-partial/` (stopped after its first repeat once the
next gap was clear). Link normalization recovered the budget-turn answers:
repeat 1 had 5/8. bug-fix became `conditional` (the run never obtained an
accepted receipt); data-processing became `completed` but only cited
`summary.txt` and had no flip condition; checksum still had no receipt or
verified artifact. The final code therefore derives claims from `read_file`
sources named in the conclusion and adds one flip condition to the plain-text
fallback.

## Results

Full product eval N=3 (`product-eval/report.json`, binary `379e8cc3…`):

| Metric | Before (acceptance fix) | After |
|---|---|---|
| Overall | 19/24 = 79.16% | **22/24 = 91.66%** |
| pass^3 | 3/8 = 37.5% | **7/8 = 87.5%** |
| pass at least once | 8/8 | 8/8 |

Per case: config-retries, data-processing-sales, research-compare,
long-task-plan, broken-json, checksum-file and directory-summary are 3/3.
bug-fix-last-index is 1/3: two runs returned a valid answer but never obtained
an accepted receipt before the step budget, so the completion gate
(deliberately) downgraded them to `conditional`. That is the acceptance
contract, not an answer-format failure.

Fast product eval N=3 is receipt-scored (a fast delivery counts as completed
only when its post-run receipt is accepted): **24/24 = 100%, pass^3 8/8,
pass at least once 8/8**. Every case passed the suite rubric against the
receipt's derived claims and sources.

## Default policy

**Decision:** `--fast` is the low-risk default candidate, but stays opt-in in
this release. It is the right profile for read-only or single-artifact local
tasks: 24/24 neutral and 24/24 post-run acceptance against Pi in the earlier
N=3 run, and 24/24 receipt-scored product cases here. Pages, write tasks that
need in-run acceptance, and high-risk work keep Core 7/full. Promotion to a
true default requires an automatic queue drain (or a TUI accept action), so a
user never ends with a pending delivery and no follow-through.

## Delivery view

- `1agent accept --list [--json]` lists pending deliveries and recent
  receipts without draining the queue.
- TUI `/status` now shows 待处理/已验收 counts with accepted/conditional split.
- Tests: `tui_status_reports_the_delivery_queue`,
  `tui_status_screen_includes_delivery_state`.

## Gates

- fmt, strict clippy, lib 1444/0/4, bin 71/0, core eval 8/8.
- `scripts/verify.sh` still stops at the pre-existing
  `cli_tui_agents_refresh_uses_configured_profiles`; it fails identically on a
  clean `8e1dfc2` worktree.
- Full eval binary `379e8cc3…`; final installed binary `233ec7a6…`. The only
  difference is the fast receipt's input-source merge, which does not run in
  the full (non-fast) eval.

## Limits

- 8 tasks × N=3: directional, not a broad quality claim.
- Fast scoring uses the post-run receipt's derived claims; it is not a human
  review, and it does not prove open-ended task quality.
- bug-fix-last-index still exposes an acceptance-contract gap: the model can
  finish a valid answer after using every step without calling
  `acceptance_gate`. Fixing that is a budget/prompt decision, not answer repair.
