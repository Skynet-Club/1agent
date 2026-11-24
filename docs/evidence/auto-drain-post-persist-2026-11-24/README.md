# Post-persist full product eval — 2026-11-24

The host `skynet-term` cap was cleared before this run (detached sessions
128 → 18), restoring `agent_exec` and 1Agent `terminal_create`.

Binary: `~/.local/bin/1agent` sha256 `eb9e6d54…` (post-`/fast` persistence,
pre-verify-green). Suite `evals/real-tasks-v1.json`, N=3, `deepseek-flash` via
`127.0.0.1:8787`.

## Result

- **22/24 attempts (91.66%)**, **pass^3 6/8 (7500 bps)**, at least once **8/8**.
- Failure classes: `evidence` 1, `generation` 1.
- Per case: bug-fix 3/3, config 3/3, data-processing 3/3, research-compare 2/3,
  long-task 3/3, broken-json 3/3, checksum-file 2/3, directory-summary 3/3.

## Misses

- `research-compare` repeat-1: expected `conditional`, got `completed` with an
  unresolved conflict and high-impact unknowns. This is an answer-generation
  contract miss, not an acceptance-gate failure.
- `checksum-file` repeat-1: the model obtained an **accepted** gate receipt but
  spent 8 tool calls on two invalid retries (media type, terminal session
  argument) and had no provider attempt left for the final tool-free answer.
  The run ended `needs_evidence` with 0 claims/evidence. The other two
  checksum runs completed normally. This is a step-budget finalization edge:
  an accepted receipt exists but the budget answer does not use it.

The earlier 24/24 full eval was on binary `ef3f0d69…`; this 22/24 is a small-N
comparison and within the prior variance range, but the checksum budget edge is
a concrete follow-up.

## Evidence

- `report.json` — full eval report.
- `stderr.log` — empty.
- `product-eval/` — per-repeat `cases.jsonl` and journals; repeat directories
  are ignored by Git and kept locally for audit.
