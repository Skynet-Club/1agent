# Evidence — Answer style v2 (language clause)

Date: 2026-11-04
Method: the same 14 objectives as the baseline, replayed after the language clause
was added to the system prompt. Paired by objective.
Command: `python3 scripts/answer_style/compare.py .journal .journal-style-post2`
Status: post-change measurement, one run per task, single model.

## 1. Paired result

| metric (median over the same 14 tasks) | baseline | now | change |
|---|---:|---:|---:|
| prose chars (code excluded) | 413 | **282** | −32% |
| sentences | 11.0 | **6.5** | −41% |
| longest restatement | 4.5 | 4.0 | −0.5 |
| answers restating > 8 chars | 4 / 14 | 1 / 14 | −3 |
| answers with 2+ enumerated options | 6 / 14 | 2 / 14 | −4 |
| answers ending in a summary | 1 / 14 | 0 / 14 | −1 |
| answers written in the wrong language | 0 / 14 | **0 / 14** | fixed |

Per-task prose delta: median **−77** chars; 10 of 14 answers got shorter.

## 2. The language clause worked

The previous round produced one English answer to a Chinese request. After adding:

> Reply in the same language as the user's request, including code comments you add.

the same task now answers in Chinese. Measured: latin-dominant answers `1 → 0`.

## 3. A measurement bug found while checking it

The first detector compared CJK characters to **Latin letters**. An English word is
several letters long, so a Chinese answer that mentions `Python`, `C`, `Java`,
`Hello` and `World` scored 0.38 and was flagged as English. It was not.

The detector now compares CJK characters against Latin **words**. Re-running the
previous round still reports one English answer, so the earlier diagnosis was
correct and the fix is real; only the new round's false positive disappeared.

This is the second time a metric has been wrong in a way that could have produced
a wrong conclusion. Both were caught by inspecting the text behind the number.

## 4. Menus are still low

2 of 14 answers contain a list of options, down from 6 at baseline. The action
menu now exists in the TUI, but the model itself no longer volunteers lists. That
is expected and acceptable: options should come from the command registry, not
from prose. It is not evidence of a problem, but it does mean "the model offers
choices" is no longer the mechanism.

## 5. What this does not show

- n = 14, one run per task, one model, one temperature. No variance estimate.
- Several objectives are malformed smoke text; one contains a pasted prior answer.
- Metrics remain regex proxies for "would deleting this line change the action".
- The same 14 tasks have now been used for three rounds. They are a tuning set,
  not a held-out set.
