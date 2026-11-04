# Evidence — Answer style v1 (paired replay)

Date: 2026-11-04
Method: same 14 objectives, replayed through the CLI with the Occam prompt.
Comparison paired by objective, not by sample.
Command: `python3 scripts/answer_style/compare.py .journal .journal-style-post`
Status: post-change measurement, one run per task, single model.

## 1. What was run

The 14 distinct real objectives from the baseline were extracted with
`baseline.py --list-objectives`, replayed via `1agent run` against the same
provider, and written to `.journal-style-post`. 14 runs, 14 successes, no
fallback answers.

## 2. Paired result

| metric (median over the same 14 tasks) | before | after | change |
|---|---:|---:|---:|
| prose chars (code excluded) | 413 | **238** | −42% |
| sentences | 11.0 | **5.5** | −50% |
| longest restatement | 4.5 | 4.5 | unchanged |
| answers restating > 8 chars | 4 / 14 | **1 / 14** | −3 |
| answers with a summary ending | 1 / 14 | 0 / 14 | −1 |
| answers with scaffolding | 0 / 14 | 0 / 14 | 0 |
| answers with 2+ enumerated options | 6 / 14 | **1 / 14** | −5 |

Per-task prose delta: median −44 chars across all pairs, −65 excluding one
malformed pair (§3). Nine of fourteen answers got shorter.

## 3. Three things the aggregate hides

**① One "pair" is not a pair.**

The first objective contains a *pasted previous answer* inside the request text,
so "before" answered 130 characters of it and "after" explained the pasted code
line by line at 732. This is corpus damage, not a regression. Excluded from the
secondary median.

**② Language is not pinned, and it broke.**

One answer replied in English to a Chinese request. Nothing in the system prompt
constrains the reply language; the old run happened to choose Chinese. This is a
real regression and needs an explicit clause, not luck.

**③ Menus collapsed from 6 to 1 — the opposite of the intended interaction.**

The Occam rules say "at most three alternatives with a named default". The model
read that as "give fewer options" and mostly stopped offering next steps. But the
product requirement is the reverse: **offer a selectable next step whenever there
is a real fork**. The clause is not wrong; it is incomplete, and `next_actions`
is not wired, so nothing supplies valid options yet.

## 4. Verdict

- Brevity: improved, clearly and on the same tasks.
- Restatement: improved on the tail (4 → 1), unchanged at the median.
- Menus: **regressed**. This must be fixed before the interaction style is
  claimed.
- Language: **regressed**. One clause fixes it.

## 5. What this evidence is not

- n = 14, one run per task, one model, one temperature. No repetitions, so no
  variance estimate.
- The objectives are one user's smoke sessions, several of them malformed.
- Metrics remain regex proxies for "would deleting this line change the action".
- The prompt also changed between the two samples, so this measures the prompt
  change, not the product.

## 6. Next, in order

1. Add a language clause: reply in the language of the request.
2. Add the menu clause **after** `next_actions` is wired, so options are real
   commands rather than free text. Writing the clause first would recreate
   unvalidated menus.
3. Re-run this comparison with three repetitions per task.
