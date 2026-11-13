# Evaluation Suites

V0 evaluation is intentionally offline and deterministic. The default `core-v3` suite contains
eight cases covering evidence success, authorization boundaries, honest refusal, missing-file
and directory failure, hidden-control rejection, zero-width control rejection, and the
sleep/honesty path. The case verdicts are repeatable; the report digest includes per-case
duration and therefore is not byte-identical across runs.

Suites declare a `kind`. `contract_fixture` suites validate system contracts and cannot be used as
model-quality promotion evidence. `model_quality` requires real provider runs and reviewed tasks.

`model-quality-template.json` is intentionally `template` status and cannot run. A runnable
model-quality suite must be `ready` and declare provider requirements, a reviewer and calibration.

| Suite | Evidence |
|---|---|
| Domain contract | Mode inference, answer validation, read-only action rejection |
| Journal | Contiguous replay, paired tool calls, terminal reason detection |
| Tool safety | Read-root containment, unknown tool handling, UTF-8 byte-limit handling |
| Provider adapter | Local HTTP fixture verifies OpenAI-compatible request/response translation; ignored in managed sandboxes that forbid loopback binding |
| End-to-end CLI | Real process run, evidence read, journal audit and replay |
| Epistemic output | Unknown and flip-condition minimums |
| Epistemic impact | High-impact unknown reduction requirements |

These tests prove contract and runtime integrity, not real-model reasoning quality. Real-model evaluation begins after provider credentials, a gold task set and calibrated human review are available.

## Golden set v1

`golden-set-v1.jsonl` is a fixed 40-prompt inventory for M19–M23 review: eight prompts each for chat, fact, decision, research and execution. Validate it with:

```bash
1agent golden-check --input evals/golden-set-v1.jsonl --json
```

The validator checks the inventory only: 30–50 entries, unique ids and prompts, safe ids, control characters and category coverage. It does not call a model and does not produce labels.

Blind-review procedure:

1. Run each prompt against 1Agent and the reference (for example Hermes) with the same provider, temperature, budget and task permissions.
2. Remove model identity and randomize pair order.
3. Two reviewers label first-answer usability, pairwise preference, over-structure, unnecessary follow-up and follow-up adoption.
4. Feed the labels to `1agent answer-quality --input LABELS.jsonl`; zero denominators stay `null`.
5. Archive the prompt version, provider version, random seed and raw journals with the report.

Changing a prompt after labels exist requires a new version and a CHANGELOG/ADR entry so prior labels remain comparable.

## Real-task suite v1

`real-tasks-v1.json` declares eight real tasks with `kind = model_quality` and `execution_status = ready`: off-by-one bug fix, config change, JSONL aggregation, two-file research comparison, three-step planning, invalid-JSON handling, checksum write and directory summary. Cases that must modify the workspace set `"allow_write": true`; every other suite stays read-only.

Validate the suite structure without a provider:

```bash
1agent eval --suite evals/real-tasks-v1.json --validate-only --json
```

Running it requires a real provider with tool calling and a human reviewer; the deterministic mock must not be used. A green validation is not a benchmark result.

## Intelligent-loop suite v1 (AG-23f)

`agent-loop-v1.json` is the fixed 100-question 0.97 release-gate inventory: 20 self-introduction, 20 current/local, 20 follow-up-reference, 20 simple-chat and 20 high-risk-action prompts.

```bash
1agent eval --suite evals/agent-loop-v1.json --validate-only --json
1agent eval --suite evals/agent-loop-v1.json --config configs/pi-deepseek.json \
  --allow-network --artifacts-dir docs/evidence/agent-loop-v1-2026-11-13 --json
```

The suite checks more than the answer status:

- `required_tools` / `required_any_tools` / `forbidden_tools` and
  `min_tool_calls` / `max_tool_calls` measure tool selection;
- `max_provider_calls` measures the simple-chat budget;
- `max_cost_micros` bounds per-case cost;
- `accepted_answer_statuses` / `accepted_stop_reasons` allow `needs_evidence` or
  `needs_authorization` where those are the honest outcomes;
- `--artifacts-dir` keeps every case journal under `journals/<case-id>/` and
  appends one `cases.jsonl` record per case, including the journal path.

Self-introduction cases require `describe_self`; current/local cases require a
web or local tool; simple chat allows zero tool calls and one provider call;
high-risk cases forbid write and terminal tools. A green run still needs human
review before it is release evidence.

