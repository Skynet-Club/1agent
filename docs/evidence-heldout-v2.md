# Evidence — Held-out v2

Date: 2026-11-04
Model: `deepseek-flash`, temperature 0.7, 3 repetitions
Corpus: `fixtures/reframing/heldout-v2.json` — 14 cases, 7 `means` / 7 `end`, 4 near-misses
Frozen before the run: `dd444f9629f73305...`
Prompt: v3, `650588186c30dffe...`, unchanged after the run
Status: **held-out result. Passes the pre-registered threshold.**

## 1. Result

| metric | v2 on heldout-v1 | v3 on heldout-v2 | threshold |
|---|---:|---:|---|
| recall on `means` | 1.000 | **1.000** (21/21) | — |
| **precision on `means`** | 0.778 | **0.955** (21/22) | ≥ 0.80 |
| over-reframe rate on `end` | 0.286 | **0.048** (1/21) | — |
| classification accuracy | 0.857 | **0.976** (41/42) | — |
| options per question | 1.93 | 3.00 when asked | 2–3 |
| yes/no question share | 0.000 | 0.000 | 0 |
| schema parse failures | 0/42 | 0/42 | 0 |
| cost | 0.027 USD | 0.024 USD | — |

Artifacts:

```text
docs/evidence/reframing-heldout-v2/FREEZE.txt       b0d1f1b966364b91...
docs/evidence/reframing-heldout-v2/responses.jsonl  f0f2c0476a62ec08...
docs/evidence/reframing-heldout-v2/summary.json     45e4dc2c68af04cc...
```

## 2. What changed between the two held-out rounds

Only the prompt. v3 replaced the single-sentence rule with a two-step test:

```text
Step 1. What is this for? If the answer is already in the request, it is an END.
Step 2. If not: would two different answers lead to materially different advice?
        If not - because the request is already fully specified, or because the
        use would not change what you recommend - it is an END.

Only when both conditions hold is it a MEANS. Naming a product or an action is
not enough on its own.
```

## 3. The one remaining failure

| case | what it asked | reps |
|---|---|---|
| `h2-end-movie-06` "订两张下周三晚上八点的电影票" | 这两张票主要是为了哪种场合？ | 1/3 |

This is the same class as heldout-v1's `h-end-ticket-03` (2/3), which v3 cut to
1/3 on a new instance. The class is not eliminated, only reduced.

Notably, the other v1 failure class — a fully specified mechanical change
(`h-end-rename-07`) — passed cleanly here on a new instance
(`h2-end-port-07`, 3/3 silent).

## 4. Caveats that must travel with this number

1. **Second-order contamination.** v3 was written after seeing heldout-v1 fail.
   heldout-v2 uses new domains and new instances, but the *failure classes* were
   known. This is a legitimate held-out run, not a pristine one.
2. **Constructed corpus.** 28 cases total across two held-out sets, all written
   by one author. No external labelers, therefore no κ.
3. **One model, one provider, one temperature, 3 repetitions.**
4. **Mechanical only.** Nothing here shows the questions are the right ones, or
   that a user would accept a proposed substitute. That is the semantic round,
   still unbuilt.
5. **No cost/benefit shown.** Even a correct reframe is worthless if the user
   would have been faster buying the drill.

## 5. What may be claimed

> On two 14-case constructed held-out sets, with prompt v3, this provider asked
> a 3-option question on every `means` case (42/42), and stayed silent on 20 of
> 21 `end` cases, with zero schema failures in 84 calls.

That sentence is defensible and bounded.

## 6. What may not be claimed

- "It understands what users need."
- "It won't over-reframe." (1/21, and the class is not eliminated)
- "It generalises to real users." (no real-user data, no labelers)
- "It makes users faster or cheaper." (no outcome measurement)

## 7. Budget

Cumulative spend across all rounds: **0.156 USD** of the 0.20 USD announced.
Further live runs need a new cap.
