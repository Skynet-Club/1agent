# Clarification gate (2026-11-20)

Status: **landed; unit negatives and one live DeepSeek round-trip pass**

## What landed

The planner may stop before decomposition when a material ambiguity cannot be
resolved by reading the sources. The model decides when to ask; the runtime
only validates and surfaces the structured answer.

- The model returns one `AgentAnswer` with `status: "needs_authorization"`,
  a short `conclusion`, and 1..=3 questions in `unknowns[].description`.
  No `PlanProposal` is produced.
- `ModelPlanGenerator` and `ModelProductionPlanGenerator` detect that answer
  before plan parsing and return `MasterPlannerError::NeedsClarification`.
- The CLI prints `{"status":"needs_clarification","plan_id":...,"questions":[...]}`
  with exit 0. Re-running with repeatable `--clarification "answer"` puts the
  answers into `PlanningContext.clarifications`, which the model receives.
- Answers are bounded: at most 8 total, each non-empty, control-free and at most
  2,000 characters.
- The master prompt asks only for material ambiguities that change the goal, a
  deliverable, a constraint, acceptance or an external action. A reversible,
  low-risk assumption is not a reason to ask; the model proceeds and records it.
  Questions already answered in `context.clarifications` must not be repeated.
- The TUI `/plan` path shows the questions instead of an opaque
  `计划生成失败` message. There is no in-place answer-and-rerun UI yet.

No keyword list decides ambiguity; the gate is model-driven and provider-neutral.

## Unit and negative tests

`cargo test --lib master_planner` — 28 passed; full lib 1,327 passed / 0 failed /
4 ignored; MAST fault injection 7 passed; `cargo clippy --tests` clean.

New deterministic tests:

- `planner_stops_before_decomposition_with_bounded_questions`: four model
  questions are capped at three; the result is `NeedsClarification`, not a plan;
  the clarification answer is present in the model-visible context.
- `planning_context_rejects_invalid_clarifications`: empty, control-containing
  and over-count answers are rejected before any provider call.
- `production_planner_surfaces_clarification_before_decomposition`: the
  production planner path applies the same gate.
- `planning_prompt_asks_only_material_questions`: the prompt contains the
  `needs_authorization` contract and the `context.clarifications` rule.

Existing clear-task tests still return a plan, so the gate does not force a
question when the model has enough information.

## Live DeepSeek round-trip

Provider `deepseek`, model `deepseek-flash`, debug binary sha256
`041fc21a…`. Workspace was empty (`/tmp/clarification-live/workspace`).

Task:

> 请你把上个季度的销售数据整理成一份给管理层的报告，下周董事会要用。

First run: `needs_clarification`, 3 questions:

1. Where is last quarter's raw sales data (path, Lark/Feishu URL, or public URL)?
   The workspace is empty, so it cannot find the data itself.
2. Which deliverable: PPTX, Markdown/Word report, or report + charts? Which
   dimensions (region/product/channel), and are YoY/QoQ required?
3. What exact date range and fiscal-year definition is "last quarter"?

Re-run with three `--clarification` answers (no real data; produce a template
and field checklist; Markdown + chart list; natural Q3 2026). The second plan:

- 1 Work Item, 1 deliverable, 2 expected artifacts;
- uses the supplied Q3 2026 / Q2 2026 / Q3 2025 periods;
- explicitly avoids inventing values and plans placeholder-only templates;
- `approved: false` (a separate approval is still required).

The re-run journal contains the three answers in the model-visible context
(four occurrences including the repair turn).

## Installed release smoke

Release binary `target/release/first-principles-agent` sha256 `045bccdd…` was
installed as `~/.local/bin/1agent`; the previous `aa6c9c8a…` build is backed up
as `1agent-pre-clarification-2026-11-20`. The same ambiguous task on an empty
workspace returned `needs_clarification` with 3 questions through the installed
binary, exit 0. Re-running the installed binary with the three answers produced
1 Work Item / 3 expected artifacts, unapproved, exit 0. See
`release-response.json` and `release-answered-plan.json`.

## What this does not prove

- One live sample per model; model judgment varies run to run. This is evidence
  for the mechanism, not a model-quality benchmark.
- The runtime cannot prove that a question was truly unknowable from tools. The
  prompt, the source-read gate and the negative tests constrain behavior; only
  evaluation over many tasks would measure how often the model asks
  unnecessarily or fails to ask.
- The live re-run plan proposed `allow_write: true`. It is unapproved and was
  not executed; write Work Items still require the explicit approval path.
- The TUI `/plan` surface displays questions but has no structured answer input
  yet. The CLI `--clarification` path is the complete round-trip.

## Reproduce

```bash
mkdir -p /tmp/clarification-live/workspace
target/debug/first-principles-agent master plan \
  --task "请你把上个季度的销售数据整理成一份给管理层的报告，下周董事会要用。" \
  --config ~/.1a/config.json \
  --workspace /tmp/clarification-live/workspace \
  --planner-journal-dir /tmp/clarification-live/journal --json
```

Then re-run with three `--clarification "..."` answers.

## Evidence

- `clarification-response.json` — first-run stdout (questions, status).
- `answered-plan.json` — re-run plan produced with the answers.
- `release-response.json` — installed release binary first-run stdout.
- `release-answered-plan.json` — installed release binary re-run plan.
- `metrics.json` — counts and hashes.
- `sha256.txt` — response, plan, journals and binary digests.
