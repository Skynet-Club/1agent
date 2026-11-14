# Evidence — Held-out v1

Date: 2026-11-04
Model: `deepseek-flash`, temperature 0.7, 3 repetitions
Corpus: `fixtures/reframing/heldout-v1.json` — 14 cases, 7 `means` / 7 `end`, 5 near-misses
Frozen before the run: `2cf887a42f135182...`
Prompt: v2, `71c640a0c60f2adbd...`, unchanged after the run
Status: **held-out result. This is the first number that counts.**

## 1. Result

| metric | value | protocol threshold | verdict |
|---|---:|---|---|
| recall on `means` | 1.000 (21/21) | — | — |
| **precision on `means`** | **0.778 (21/27)** | ≥ 0.80 | **not met** |
| over-reframe rate on `end` | 0.286 (6/21) | — | — |
| classification accuracy | 0.857 (36/42) | — | — |
| mean options per question | 1.93 | 2–3 | met |
| yes/no question share | 0.000 | 0 | met |
| schema parse failures | 0/42 | 0 | met |
| latency p50 / p95 | 3225 / 5463 ms | — | — |
| cost | 0.027 USD | guard 0.20 | — |

Artifacts:

```text
docs/evidence/reframing-heldout-v1/FREEZE.txt        e8adebafa040aa93...
docs/evidence/reframing-heldout-v1/responses.jsonl   5d871dc937fc0fe9...
docs/evidence/reframing-heldout-v1/summary.json      91148a463bdcfc50...
```

## 2. It fails exactly on the near-misses

All 7 `means` cases were correct, 3/3. All 6 failures were on `end` cases that
look like purchases or actions:

| case | what the model asked | 3 reps |
|---|---|---|
| `h-end-buy-salt-02` "帮我买包盐" | 你买盐主要是为了什么用途？ | 3/3 |
| `h-end-ticket-03` "订一张明天上午去上海的高铁票" | 你去上海主要是为了商务办事、旅游探亲，还是中转？ | 2/3 |
| `h-end-rename-07` "把所有 user_id 重命名成 account_id" | 这次重命名，首要目标是哪一类？ | 1/3 |

These are the cases where asking "what is it for?" produces an answer that
**changes nothing**. The rule has two conditions, and the model applies only the
first:

1. the request names a means, **and**
2. two different answers would lead to materially different advice.

Condition 2 is where it fails. It is not a knowledge gap; it is the missing half
of the rule.

## 3. What the tuning set hid

On `pilot-v0` the same prompt scored precision-equivalent 0.955. The difference
is entirely in corpus composition: the pilot had 3 `end` cases, and none of them
were trivial purchases. The held-out set has 7 `end` cases, 4 of them
deliberately near-miss.

This is the concrete reason the pre-registration exists. Without it, the pilot
number would already be in the README.

## 4. Verdict against the protocol

- Initial threshold `precision_of_means ≥ 0.80`: **not met** (0.778).
- Kill condition `precision_of_means < 0.60`: **not triggered**.
- Therefore: the capability is real but not yet usable as a default.

The product claim in `docs/product-manual.md` §4 ("默认不重构") remains
**unproven**. `docs/claims.md` keeps it at ❌.

## 5. What may be claimed

- With prompt v2, this provider asks a 2–3 option, non-yes/no question on every
  constructed `means` case (21/21) with zero schema failures.
- It does **not** yet reliably distinguish "instrumental" from "trivial" requests.

## 6. What must not be done next

Do **not** tune the prompt on `heldout-v1` and re-run it. That would destroy the
only clean number in the project.

The correct next moves, in order:

1. Fix the prompt using the **pilot** set (tuning) only, adding condition 2
   explicitly as an operational test: "would a different answer change what you
   recommend? if not, it is an END."
2. Write a **second** held-out set, frozen, different domains again.
3. Run once, record whatever happens.
