# Real-Task Benchmark — Evidence (2026-11-12)

Status: **local real-provider runs; calibration required**
Suite: `evals/real-tasks-v1.json` (8 tasks)
Provider: DeepSeek `deepseek-flash` via `configs/pi-deepseek.json`
Temperature: 0.0 (provider request default)
Timeout: 240s per case

## Commands

```bash
export PI_DEEPSEEK_API_KEY="$(jq -er '.deepseek.key' ~/.pi/agent/auth.json)"
1agent eval --suite evals/real-tasks-v1.json \
  --config configs/pi-deepseek.json --allow-network \
  --timeout-seconds 240 --json > <report>.json
```

Raw reports:

```text
docs/evidence/real-tasks-benchmark-2026-11-12.json       # run 1, before calibration
docs/evidence/real-tasks-benchmark-2026-11-12-run2.json  # run 2, after calibration
docs/evidence/real-tasks-benchmark-2026-11-12-run3.json  # run 3, repeatability
```

## Suite calibration (2026-11-12)

The first real run exposed two suite defects, fixed before run 2:

1. `expect_checkpoint` defaulted to `false`, but every task uses tools and
   therefore creates a checkpoint. All eight cases now set
   `expect_checkpoint: true`.
2. The first objective required `saturating_sub` in the rubric but did not say
   so in the objective. The objective now names `saturating_sub`.
3. Required evidence sources now match a workspace absolute path that ends in
   the required relative name (`/tmp/ws/sales.jsonl` matches `sales.jsonl`).
   Failure messages include the actual sources.

The suite is still marked `calibration_required: true` and names a human
reviewer; these automated runs are not the final calibration.

## Run 1 (before calibration)

```text
total=8 passed=3 failed=5 hard_gate_failures=0 rubric=11/18
decision_completeness=10000 bps (8/8 ok)
issues: missing evidence sources (absolute-path mismatch), rubric term not
stated in the objective, checkpoint expectation false
```

## Run 2 (after calibration)

```text
total=8 passed=6 failed=2 hard_gate_failures=1 rubric=16/16
decision_completeness=8750 bps (7/8 ok), floor 8000 passed
wall clock=71.6s, p50=9.5s, max=16.6s
tokens: input=105,543 output=12,345, cost_micros=0 (provider pricing not embedded)
```

Failures:

- `research-compare`: returned `conditional` instead of `completed`.
- `checksum-file`: hard gate failure — invalid model output, conflict `x1` has
  invalid or duplicate claim references; automatic repair did not produce a
  valid answer. The temp journal was not retained.

## Run 3 (repeatability)

```text
total=8 passed=5 failed=3 hard_gate_failures=0 rubric=18/18
decision_completeness=10000 bps (8/8 ok)
wall clock=76.6s, p50=9.7s, max=12.7s
tokens: input=122,180 output=12,844, cost_micros=0
```

Failures:

- `research-compare`, `long-task-plan`, `checksum-file`: returned
  `conditional` instead of `completed`. All rubric checks passed.

## Run 4 (after the repair/source prompt fix)

```text
total=8 passed=6 failed=2 hard_gate_failures=0 rubric=18/18
decision_completeness=10000 bps (8/8 ok)
wall clock=104.2s, p50=11.9s, max=25.0s
tokens: input=121,000 output=17,000 (approx., per-case sum)
```

Failures: `research-compare` and `checksum-file` returned `conditional`
instead of `completed`; all rubric checks passed and the earlier hard-gate
validation failure did not recur.

## Post-calibration aggregate (runs 2 + 3 + 4)

```text
runs: 3
task runs: 24
passed: 17 (70.83%)
hard-gate failures: 1 (checksum-file, run 2)
rubric: 52/52
decision completeness: 7/8, 8/8, 8/8; all above the 8000 bps floor
```

Per task:

| Result | Tasks |
|---|---|
| Stable pass (3/3) | bug-fix-last-index, config-retries, data-processing-sales, broken-json, directory-summary |
| Flaky (2/3) | long-task-plan |
| Stable fail (0/3) | research-compare (conditional), checksum-file (conditional; one hard gate in run 2) |

Run 1 is excluded from the aggregate because the suite still had the
checkpoint and path-matching defects.

## Reliability follow-up (2026-11-12)

The `checksum-file` hard-gate failure was a runtime validation failure, not a
rubric failure: the model produced a derived/assumed claim without a scope or a
conflict with invalid claim references, and the automatic repair did not return
a valid answer. Three manual reproductions after the fix:

```text
attempt 1: conditional, evidence source /tmp/.../checksum.txt, claims verified,verified,assumed,verified
attempt 2: conditional, evidence source /tmp/.../checksum.txt, claims verified,verified,assumed
attempt 3: conditional, evidence source /tmp/.../checksum.txt + task objective, claims verified,verified,derived,assumed
```

The repair prompt now restates the evidence/claim/conflict invariants (source
declaration, existing claim ids, scope for derived/assumed, two sources for a
conflict, empty model-prior sources, distinct conflict pair). The system prompt
now prefers a concrete file path or URL as `evidence.source` over a tool/session
id. The validation hard failure did not recur in these three runs; the model
still chooses `conditional` because it carries an assumed claim.

## What this proves

- The real-task suite can run against a real provider with tool calling and
  write permissions; audit stays clean for the passing cases.
- The suite now has a fair checkpoint expectation and path-insensitive
  evidence-source matching.
- Task success, duration and token usage can be measured per case.

## What this does not prove

- M1's 8-hour long-horizon success, M2 autonomy hours or M3 interventions/task;
  this is an 8-task benchmark, not an 8h run.
- Final model quality: in-suite `calibration_required` and the human reviewer
  requirement remain unmet. A single provider/model is sampled.
- Cost per task: `cost_micros` is 0 because provider pricing is not embedded;
  token counts are measured instead.
- Repeatability beyond the runs recorded here; run 3 is the repeatability
  sample.
