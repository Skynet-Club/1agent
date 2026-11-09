# Evidence — Answer style v2 set (second held-out)

Date: 2026-11-04
Corpus: `fixtures/answer_style/style-v2.json`, 14 tasks in domains unrelated to
style-v1, frozen at `2faf94685b31b755eb3571b989ea6d7917f241a4945226483f63f018d5657fe7`
before the first call. Run once.
Status: second independent style measurement, same pre-registered targets.

## 1. Targets against actual

| metric | target | style-v1 | style-v2 | verdict |
|---|---:|---:|---:|---|
| prose chars, p50 | ≤ 350 | 292.5 | **222.5** | pass |
| sentences, p50 | ≤ 8 | 4.0 | **5.0** | pass |
| answers restating > 8 chars | ≤ 4 / 14 | 2 | **2** | pass |
| wrong-language answers | 0 | 0 | **0** | pass |
| answers with scaffolding | 0 | 0 | **0** | pass |

Distribution: prose p90 659, max 9556; sentences p90 20, max 160.

The maximum is the task that asked for an arbitrarily long Kubernetes
explanation. It was answered at 9556 characters, which is the correct response
to that request, not a violation.

## 2. The two traps

**"给我 10 个提高代码质量的方法"** — asked for ten.

Answered with a judgement first: "先做「测试网 + CI 门禁」这两条，其余八条才有落地
土壤", then the ten ranked by leverage. It complied with the request and still
gave a default. That is the behaviour the contract wants: brevity rules may not
turn into refusing what was asked for.

**"我们应该把单体拆成微服务吗"** — a yes/no question.

Answered "默认别拆: 先把单体整理成模块边界清晰的模块化单体, 只有当某个模块已经出现
具体的、结构性的发布/伸缩/隔离痛点时…" in 287 characters, no menu. A judgement
with one revision condition, not a list of considerations.

## 3. What this adds

style-v1 could have been a lucky draw: 14 tasks written by one author, one run.
A second set in different domains, with the same targets, removes some of that
risk. Both pass all five.

It does not remove the main limitation: the same author wrote both sets and set
the thresholds.

## 4. What this does not show

- n = 28 across both sets, one run per task, one model, one temperature.
- No comparison against another assistant; this is shape, not relative quality.
- `padding_rate`, the contract's real measure, is still not computed: it needs a
  human judgement per sentence.
- Style-v1 and v2 share an author, a model and a prompt. A different model could
  pass different style with the same score.
