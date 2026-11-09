# Evaluation Suites

V0 evaluation is intentionally offline and deterministic. The default `core-v2` suite contains
six cases covering evidence success, authorization boundaries, honest refusal, file failure and
hidden-control rejection.

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
