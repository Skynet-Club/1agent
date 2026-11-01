# Status v0.46.0

Date: 2026-11-02
Status: model-quality suite template and execution policy implemented

## Changes since v0.45.0

- Added evaluation execution status:
  - `unspecified`;
  - `template`;
  - `draft`;
  - `ready`.
- Added model-quality prerequisites:
  - non-empty provider requirements;
  - reviewer;
  - calibration required.
- Added `evals/model-quality-template.json`.
- Template suites may contain zero cases and load for inspection.
- Template and draft suites cannot run.
- Model-quality suites cannot run or promote unless status is ready and prerequisites are present.
- Contract fixture behavior remains unchanged.

## Verification

| Check | Result |
|---|---|
| template loads | passed |
| template execution | rejected |
| core contract suite | 6/6 passed |
| model-quality missing metadata | rejected |
| legacy unspecified suites | compatible |
| Unit tests | 82 passed |
| Clean-source package gate | passed |

## Verified clean release

```text
source commit: 32a517f20e8a744785f50966e7dc57b385614e29
git dirty: false

Linux binary:
fa047eb46c27ec59e10cf94a66d8a748804d5d8f76344534a9aacbdd2b10108d

Windows binary:
2837e17ed57d07bff0be4df3d1a4d6310797cf137ccfbcd50a4ea15cdfb964dc

Linux tar.gz:
17386882e066e3917a25e811169c8faf15de87c8bbeb016245943a47bbc2ecf4

Windows zip:
889b395149e8e3974edeef541071fb6d3d523d063126b98b57948f3ce938b9d5

Release manifest:
3004441dfe38f0a2cad855dca7097abeadc2f58d4e73dc804e26b886ecdccd06
```

## Evidence boundary

The template is not a benchmark result. It exists to define what must be supplied before a real
model-quality run. No provider/model quality claim is made.

## Next requirement

Add per-case evidence provenance requirements:

1. Require source IDs for verified claims.
2. Reject duplicate evidence references.
3. Require scope for derived or assumed claims.
4. Keep model priors explicitly source-free.
5. Test evidence provenance serialization and audit.
