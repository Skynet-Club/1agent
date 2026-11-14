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
| Clean-source package gate | passed |

## Verified clean release

```text
source commit: 7f0e85c8a6b2664921571eecca21b551d2a3d283
git dirty: false

Linux binary:
39d4bb7b246eca72aa4cf582b8fff276150f0c64bcb549ac4314173c2fdb7374

Windows binary:
09e877842f8949109c9d4fa564c1f545fbb72a510ed1255b0247112ce5d4848f

Linux tar.gz:
ee0368aad6eb46e247ac95c8d558197a5db7c66baaff9aff05321c8228cb66c0

Windows zip:
718d2f7f1d9b106ee9d2cbaa706c0c1ea1f44183e1c02f4de8662c8ef96796ad

Release manifest:
45beb54b754d320afdc36565232dfec765b14d7c3234ccc6609403c144e2b0d6
```

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
