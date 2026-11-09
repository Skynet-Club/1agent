# Evidence — Answer style baseline v0

Date: 2026-11-04
Corpus: local `.journal`, 75 runs
Command: `python3 scripts/answer_style/baseline.py`
Status: pre-change baseline. All answers were produced by the prompt that existed
**before** the Occam laws were added.

## 1. What the corpus actually contains

```text
answer events        62
distinct conclusions 21
  fallback           29 events   (deterministic-provider notice)
  evidence_error     14 events   (missing read-root)
  real               19 distinct answers
measurable real      19
```

Two thirds of the journal is not prose at all: it is the offline-provider
fallback notice or a failed file read. Only 19 distinct answers are measurable,
and they come from one user's smoke sessions.

## 2. Measurements

| metric | median | p90 | max |
|---|---:|---:|---:|
| total chars | 682 | 1380 | 1597 |
| prose chars (code blocks excluded) | 469 | 849 | 1022 |
| sentences (prose only) | 13 | 36 | 45 |
| longest verbatim overlap with the request | 5 | 11 | 11 |

| marker | count / 19 |
|---|---:|
| contains scaffolding (当然/首先/总之/希望…) | **0** |
| restates the request for > 8 chars | **8** |
| ends with a summary sentence | 2 |
| contains a list of 2+ enumerated options | **9** |

## 3. Three findings

**① Filler openings are already gone.**

Zero scaffolding hits. The existing instruction "Do not greet or repeat the
request" is doing its job at the opening. The padding problem, if any, is not at
the start.

**② Restatement is the real leak.**

8 of 19 answers quote the request back for at least nine characters. Longest
overlap reaches 11. That is the `restatement` metric in the output contract, and
it is the only marker here that is clearly failing.

**③ Menus already exist, and nothing validates them.**

9 of 19 answers contain a list of enumerated options. They are generated freely
by the model, with no check against the command registry. Under the new
interaction rule those items are promises; today they can promise anything.

This is the strongest argument for `next_actions`: the behaviour is already
happening, just unverified.

## 4. What this evidence is not

- **Proxy metrics.** The contract's real test is "would deleting this line change
  the user's next action". This script counts markers that correlate with it. It
  is not the contract metric and must not be reported as such.
- **n = 19**, one user, mostly smoke tasks. Percentiles are indicative only.
- **Pre-change prompt.** These numbers describe the old behaviour. They are the
  baseline the new rules have to beat, not a measurement of them.
- **`restatement > 8` is a weak threshold.** Reusing a domain term legitimately
  can exceed eight characters; the count is a flag for review, not a verdict.

## 5. Targets for the first post-change measurement

Provisional, to be revised only with recorded evidence:

| metric | baseline | target |
|---|---:|---:|
| prose chars p50 | 469 | ≤ 350 |
| sentences p50 | 13 | ≤ 8 |
| restatement > 8 | 8/19 | ≤ 4/19 |
| menus without a default | unknown (not measured) | 0 |

## 6. Next

1. Wire the Occam prompt into a real run and answer 20 real tasks.
2. Re-run this script; compare against §5.
3. Until then, `docs/claims.md` records the baseline as measured and the style
   rules as unproven.
