# 1AO decision/action P0 — first slice (2026-11-19)

Scope: read-only ontology export. No prompt, journal event, answer schema or
runtime behavior changed.

## Implemented

- `Decision` type and deterministic IRI
  `urn:1agent:decision:<run>:main`.
  - hashed objective (`questionSha256`, `questionChars`);
  - status from `AnswerProduced.answer.status`;
  - links to Run, Answer, Claims, Evidence and the Outcome record.
- `Outcome` type and IRI `urn:1agent:outcome:<run>`.
  - status from `RunStopped.reason` (fallback: answer status);
  - link to the Decision and generated artifacts.
- `Run.hasDecision` reference.
- `Option` nodes from `ReframeProposed` options, with selected/rejected/proposed
  status and hashed option text; `Decision.hasOption` and `chosenOption`.
- `Criterion` nodes from the reframe stop rule (verifiable, deliverable,
  better-substitute), with source asserted/derived; `Decision.hasCriterion`.
- Type-registry entries for `Decision`, `Outcome`, `Option`, `Criterion`; data
  bindings for question digest, statuses, option index and stop-rule criteria.
- Validator knows all four types and their status/field rules; unknown
  references and malformed statuses still fail closed.

## Verification

| Check | Result |
|---|---|
| `./scripts/verify.sh` | **pass**; 1653 total, 0 failed, 13 ignored |
| ontology unit tests | 31 passed |
| ontology CLI tests | 27 passed |
| real journal smoke | Decision + Outcome nodes exported and validated |
| claims | 264 (240/16/8) |

Raw: `verify-1ao-decision-p0.log`, `verify-1ao-option-criterion.log`.

## Not in this slice

- `Scenario` nodes.
- Standalone `ActionType` registry and generated typed tools.
- Scenario staging / atomic merge.
- Object/action-level policy and Ontology MCP.

## Next

1. Add decision/action competency questions and golden tests.
2. Add `Scenario` staging semantics (P2) and the code-change/release
   ActionType registry (P1, 1.2).
