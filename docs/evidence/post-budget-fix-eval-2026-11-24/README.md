# Pre-fix full product eval — 2026-11-24

This is the "before" run for the accepted-delivery fix. Binary
`~/.local/bin/1agent` sha256 `25be45d2…` (step-budget synthesis only), suite
`evals/real-tasks-v1.json`, N=3, `deepseek-flash` via `127.0.0.1:8787`.

## Result

- **21/24 attempts (87.50%)**, **pass^3 5/8 (6250 bps)**, at least once **8/8**.
- Failure classes: `evidence` 2, `generation` 1, `unknown` 1.
- Per case: bug-fix 3/3, config 2/3, data-processing 2/3, research-compare 3/3,
  long-task 3/3, broken-json 3/3, checksum-file 2/3, directory-summary 3/3.

## Misses

- `config-retries` repeat-1: expected `completed`, got `conditional`; the
  delivery was fine but the model's answer-generation contract downgraded it.
- `data-processing-sales` repeat-2 and `checksum-file` repeat-3: each had an
  **accepted** gate receipt, but the final answer was invalid and repair was
  unavailable, so `unrepairable_answer` returned `needs_evidence` with 0
  claims/evidence. The step-budget synthesis did not cover this exit.

The two accepted-receipt misses motivated extending the synthesis to the
failed-repair path and the tool-call budget. This directory is superseded by
`docs/evidence/unrepairable-accept-2026-11-24/`.

## Evidence

- `report.json` — full eval report.
- `stderr.log` — empty.
- `product-eval/` — per-repeat `cases.jsonl` and journals; repeat directories
  are ignored by Git and kept locally for audit.
