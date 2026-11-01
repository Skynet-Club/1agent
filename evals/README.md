# Evaluation Suites

V0 evaluation is intentionally offline and deterministic.

| Suite | Evidence |
|---|---|
| Domain contract | Mode inference, answer validation, read-only action rejection |
| Journal | Contiguous replay, paired tool calls, terminal reason detection |
| Tool safety | Read-root containment, unknown tool handling, UTF-8 byte-limit handling |
| Provider adapter | Local HTTP fixture verifies OpenAI-compatible request/response translation; ignored in managed sandboxes that forbid loopback binding |
| End-to-end CLI | Real process run, evidence read, journal audit and replay |

These tests prove contract and runtime integrity, not real-model reasoning quality. Real-model evaluation begins after provider credentials, a gold task set and calibrated human review are available.
