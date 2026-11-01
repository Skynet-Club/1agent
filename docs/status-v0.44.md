# Status v0.44.0

Date: 2026-11-02
Status: scripted reasoning fixture coverage implemented

## Changes since v0.43.0

- Added a deterministic scripted reasoning provider in the eval test harness.
- Added three fixture cases:
  - contradictory sources produce `conflicting` claims, unknowns and flip conditions;
  - a decision emits an explicit flip condition;
  - a provider requests file evidence and only then returns a verified conclusion.
- The external-correction case validates a real tool-call/observation/final-answer loop.
- Rubrics validate required evidence sources and claim statuses.

## Verification

| Check | Result |
|---|---|
| Fixture cases | 3/3 passed |
| Source conflict structure | passed |
| Decision flip condition | passed |
| External observation correction | passed |
| Unit tests | 81 passed |
| Core evaluation rubric | 6/6 passed, 0 hard-gate failures |

## Evidence boundary

This fixture proves that the evaluation harness can recognize the intended structures. It does
not prove that DeepSeek, Kimi, Qwen, Ollama or any real model will produce those structures.

## Next requirement

Make fixture suites visible without conflating them with model-quality runs:

1. Add a suite metadata `kind` field.
2. Distinguish `contract_fixture` from `model_quality`.
3. Include the kind in JSON/text scorecards.
4. Reject fixture suite promotion as model-quality evidence.
5. Preserve backward compatibility with existing suites.
