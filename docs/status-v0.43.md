# Status v0.43.0

Date: 2026-11-02
Status: broader offline epistemic evaluation implemented

## Changes since v0.42.0

- `EvalExpectation` adds:
  - `min_unknowns`;
  - `min_flip_conditions`.
- `EvalCaseResult` reports unknown and flip-condition counts.
- Core suite upgraded to:
  - name `core-v2`;
  - suite version `2`;
  - six cases.
- New cases:
  - missing evidence file returns a conditional/needs-evidence answer with a tool failure;
  - zero-width hidden control is rejected and not converted into evidence.
- Existing cases now assert unknown/flip-condition requirements.

## Core cases

1. verified file evidence;
2. write/execute requires authorization;
3. no reasoning provider returns needs-evidence;
4. bidirectional control rejected;
5. missing-file tool failure;
6. zero-width control rejected.

## Verification

```text
suite: core-v2
passed: 6/6
hard gate failures: 0
rubric: 6/6
```

| Check | Result |
|---|---|
| Unit tests | 80 passed |
| Core evaluation rubric | 6/6 passed |

## Known limitations

- This remains a deterministic contract/safety suite, not real-model quality evaluation.
- Source-conflict and multi-step decision quality need a scripted or real reasoning provider.
- Missing-file and hidden-control cases validate refusal behavior, not reasoning depth.

## Next requirement

Add a scripted reasoning fixture suite:

1. Build deterministic synthetic providers for reasoning patterns.
2. Cover contradicting sources and explicit conflicts.
3. Cover decision flip conditions.
4. Cover a provider that self-corrects only after external observation.
5. Keep the suite clearly labeled as fixture behavior, not model quality evidence.
