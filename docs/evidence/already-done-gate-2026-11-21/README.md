# Already-done gate live check (2026-11-21)

Goal: an exact repeat of a completed Act task whose recorded artifacts are
unchanged should ask “new version or verify?” before any model call, instead
of redoing the work.

Environment: installed binary
`e9d7d2baf749b3c1e08a5a8c0e235a6897092fc0e374628f92abcde7aee1a84f`,
workspace `/tmp/tui-cont-test`, journal `/tmp/tui-cont-journal`, TUI session
`ses_tui_cont_p_20261121` (page task).

## Gate on an exact repeat

The page task was previously completed as run `7dad9514`; `page.html` was
1097 bytes, sha256 `2b1b6b18…`.

| Run | Entry | Mode | Models | Tools | Result |
|---|---|---|---|---|---|
| `85bd9a44` | debug binary, same objective | ask | **0** | **0** | “这个任务在之前的运行 7dad9514 中已经完成，记录的产物未变：/tmp/tui-cont-test/page.html（sha256 2b1b6b18…）。要新版本，还是只验收现有产物？” |
| `ba05b7e9` | installed binary, same objective | ask | **0** | **0** | Same question |

The page file was byte-identical after both runs (`2b1b6b18…`, 1097 bytes);
the session transcript grew by one user/assistant pair each time.

## Verify-only follow-up

Reply: “只验收现有 page.html，不要重做，也不要修改任何文件。”

Run `2a36a3f1`: mode act, 11 models, tools `read_file`, `preview_file`,
`browser`×2, `acceptance_gate`×4; **0 writes**. The page stayed
`2b1b6b18…`. The acceptance gate returned accepted (7 public + 3 hidden
checks), but the final answer was **conditional** because the completion
freshness gate requires a page task to write the artifact in the same run.
The model explained this and offered “允许重写” as the way to reach completed.

## Finding

The gate itself works: exact repeat → instant question, no model, no tools,
no file change. The verify-only branch still pays a full read-only audit
(11 models) and is downgraded to conditional by the freshness gate. The next
fix is a verification-only mode: when the user explicitly verifies an
unchanged artifact and the acceptance receipt is accepted, the run should be
completed without requiring a fresh write.

## Verification-only mode

The first “只验收” reply (`2a36a3f1`) was conditional because the freshness
gate required a write in the same run. The fix:

- detect explicit verify-only intent (“只验收 / 仅验收 / verify only”, or a
  verify reply right after the gate question) together with unchanged active
  artifacts;
- inject a `VERIFY ONLY` system message, skip the page delivery checklist;
- the completion gate checks the expected artifact sha256 and skips the design
  contract gap in verify-only mode;
- a read-only browser audit no longer clears the acceptance receipt;
- the ledger keeps 20 entries for the gate (context still shows 5).

| Run | Binary | Models | Writes | Receipt | Status | Note |
|---|---|---|---|---|---|---|
| `8eb5f33b` | debug, before design-gap skip | 6 | 0 | accepted | conditional | design contract gap |
| `b6e90da6` | debug, after design-gap skip | 6 | 0 | accepted | **completed** | page unchanged |
| `5484e6cb` | debug, after browser fix | 6 | 0 | accepted | **completed** | page unchanged |
| `8d8b511a` | installed, before browser fix | 14 | 0 | cleared by a browser audit after acceptance | conditional | `completion_acceptance_missing` |
| `15e68de3` | installed, after browser fix | 13 | 0 | conditional (brief/CTA residual risk) | conditional | correct: the receipt itself is conditional |

The last row is the boundary: verification-only can return completed only when
the acceptance receipt is accepted. A receipt that is conditional for a real
brief/CTA gap keeps the answer conditional; that is not a runtime gate false
negative.

## Limits

- Exact objective match only; fuzzy/paraphrased repeats do not trigger.
- Only the last 5 ledger entries are inspected.
- Recorded artifacts must be present with the same size and sha256; reused
  paths are listed but not hashed.
- “New version” still follows the model’s path choice; versioned paths are
  not enforced yet.
