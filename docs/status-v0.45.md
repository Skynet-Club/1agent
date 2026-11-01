# Status v0.45.0

Date: 2026-11-02
Status: explicit evaluation suite kinds implemented

## Changes since v0.44.0

- Added `EvalSuiteKind`:
  - `unspecified` for legacy/unclassified suites;
  - `contract_fixture`;
  - `model_quality`.
- `core-v2` and shipped held-out examples are marked `contract_fixture`.
- JSON and human-readable reports include the suite kind.
- Experience evaluation rejects `contract_fixture` suites as model-quality promotion evidence.
- Legacy suites without a `kind` remain loadable as `unspecified`.

## Verification

| Check | Result |
|---|---|
| core kind | contract_fixture |
| report kind | passed |
| fixture promotion rejection | passed |
| legacy default | unspecified |
| Core evaluation | 6/6 passed |
| Unit tests | 81 passed |

## Evidence boundary

A contract fixture validates the harness and system contracts. It cannot establish provider model
quality. A real model-quality suite still requires configured providers, representative tasks,
reviewed references and reproducible run evidence.

## Next requirement

Add a model-quality suite template without pretending it has been run:

1. Define required provider/model metadata.
2. Define reviewer and calibration prerequisites.
3. Add explicit not-run status.
4. Prevent empty/template results from promotion.
5. Keep fixture suites allowed for CI contracts.
