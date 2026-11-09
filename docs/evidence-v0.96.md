# Evidence — Reframing Pilot v0

Date: 2026-11-04
Model: `deepseek-flash`, temperature 0.7, 3 repetitions per case
Corpus: `fixtures/reframing/pilot-v0.json` (14 cases: 11 means, 3 end)
Status: **tuning result, not held-out evidence**

## 1. What was run

```bash
python3 scripts/reframing/run_pilot.py --reps 3 --model deepseek-flash
```

42 calls per round. Two rounds: prompt v1, then prompt v2 after v1's failure was
diagnosed. Total estimated cost 0.046 USD (guard was 0.20 USD).

Artifacts, committed for inspection:

```text
docs/evidence/reframing-pilot-v0/summary-v1.json
docs/evidence/reframing-pilot-v0/responses-v1.jsonl   sha256 024a4ab1f7d215c5...
docs/evidence/reframing-pilot-v0/summary-v2.json
docs/evidence/reframing-pilot-v0/responses-v2.jsonl   sha256 9d7daa4bc4fe20c1...
```

## 2. Results

| metric | v1 | v2 |
|---|---:|---:|
| classification accuracy | 0.286 | **0.905** |
| asked a splitting question on means | 0.091 | **0.970** |
| over-reframed on end | 0.000 | 0.333 |
| mean options per question | 0.21 | 2.50 |
| yes/no question share | 0.000 | 0.000 |
| schema parse failures | 0 / 42 | 0 / 42 |
| markdown-fenced responses | 0 / 42 | 0 / 42 |
| latency p50 / p95 (ms) | 1977 / 4592 | 3070 / 5752 |
| tokens in / out | 12597 / 11664 | 18099 / 18927 |

Per case under v2:

- 10 of 11 `means` cases: 3/3 correct, always a 3-option question.
- `f6-apidoc-12`: 2/3 — unstable on "写个接口文档".
- `f5-priority-09` (`end`): 3/3 asked a question, but the question was
  "你更想优先保障哪个结果？" — a scoping question, not a needs reframe.
  The metric `asked_question` does not separate the two, so this is
  **a corpus/metric defect as much as a model error**.
- `f7-typo-13`, `f7-teameff-14` (`end`): 3/3 correctly stayed silent.

## 3. What v1 exposed

Prompt v1 said "default to END, choose MEANS only when clearly instrumental".
The model classified 10 of 11 `means` cases as END and returned empty fields.
This was a **specification error, not a capability limit**: the rule conflated the
product stance ("do not over-reframe") with a classification prior.

Prompt v2 replaced the prior with an operational test:

> ask "what is this for?"; if the answer is not in the request and two different
> answers would change the advice, it is a MEANS.

## 4. What may be claimed

- With the v2 operational rule, this provider produced a 2–3 option splitting
  question on 10 of 11 constructed `means` cases, with zero schema failures in
  42 calls, and never produced a yes/no question.
- The schema contract ("JSON only, 2–3 options, END implies null question") is
  satisfiable by a current provider without repair.

## 5. What may not be claimed

- **Not held-out.** v2 was written after seeing v1 fail on these same 14 cases.
  The corpus has therefore become a tuning set. Any number above is a tuning
  result.
- **Not about correctness.** Nothing here says the questions are the *right* ones,
  or that a user would accept the proposed substitute. That requires external
  labelers and Cohen's κ, per `docs/thesis/02-eval-protocol.md`.
- **Not about over-reframing.** Only 3 `end` cases exist, one of which is
  label-ambiguous. The 0.333 figure is not an estimate of anything.
- **Not stable.** One `means` case flipped 1/3 of the time. Variance is
  unmeasured beyond 3 repetitions.
- **Not general.** One model, one provider, one temperature, 14 constructed cases.

## 6. Next required step

Before any claim, a fresh **held-out** set is required:

1. Write 14 new cases, balanced 7 `means` / 7 `end`, especially more `end` cases
   and more near-misses.
2. Freeze it with a SHA-256 before running.
3. Run once, with no prompt edits afterwards.
4. Record the result even if it is bad.

Until then, `docs/claims.md` keeps these numbers at ⚠️.
