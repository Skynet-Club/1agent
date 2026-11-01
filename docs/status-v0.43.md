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
| Clean-source package gate | passed |

## Verified clean release

```text
source commit: 478a617e4825394217fed0f9ed6d51bfd59ac0bb
git dirty: false

Linux binary:
616a25c5cdb5b1e9d7659bf45fd885af5018d2539a6ceeb79f5e72e3b5b3c12f

Windows binary:
2ab4d4ed758315d4934627a6a62ca42c4115bdae993631a55e6dda15248e80be

Linux tar.gz:
77dc4d4fdb3af19dd91e930f91d4b6c0ad56b87256b1192dc42af1fc89e34150

Windows zip:
3ac7bdaca908b4dcc35450d3947795a3ae1fc91f7282f7ef8124e2b31b1ed9c9

Release manifest:
ce6848c319519ca2bf8248cb2ec1bf2889292a6b0ba6246ddcca4494343a90d1
```

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
