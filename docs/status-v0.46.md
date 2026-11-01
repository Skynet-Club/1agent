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
