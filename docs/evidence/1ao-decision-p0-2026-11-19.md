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
- Type-registry entries for `Decision` and `Outcome`; data bindings for
  question digest, decision status and outcome status.
- Validator knows both types and their status fields; unknown references and
  malformed statuses still fail closed.

## Verification

| Check | Result |
|---|---|
| `./scripts/verify.sh` | **pass**; 1652 total, 0 failed, 13 ignored |
| ontology unit tests | 30 passed |
| ontology CLI tests | 27 passed |
| real journal smoke | Decision + Outcome nodes exported and validated |
| claims | 264 (240/16/8) |

Raw: `verify-1ao-decision-p0.log`.

## Not in this slice

- `Option` / `Criterion` / `Scenario` nodes.
- Standalone `ActionType` registry and generated typed tools.
- Scenario staging / atomic merge.
- Object/action-level policy and Ontology MCP.

## Next

1. Add `Option` and `Criterion` derivation from clarify/plan/answer data where
   available; mark missing values `Unknown`.
2. Add decision/action competency questions and golden tests.
3. Start the code-change/release ActionType registry (P1, 1.2).
