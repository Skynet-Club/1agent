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
