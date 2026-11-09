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
| Clean-source package gate | passed |

## Verified clean release

```text
source commit: b521a302cf73bf09d0640ddba64bc708d0d9723b
git dirty: false

Linux binary:
65ac0af82131324a74f18d39e4d899a7624f3a5c2ee359ef07998dce9241f29e

Windows binary:
51bb5def5227d32d96de0d4c83ac0dffd22aa37b6eaa996487db7f0a05895b28

Linux tar.gz:
f64b27d19ac64bc651e1eafe889e7518b1c856bf35a64dbf24778e0c30991e2a

Windows zip:
d88fe7503b6c2b78b2c60c34b6616a745d9e051e1295c600172bef5fb2403821

Release manifest:
b0d43aa3362a8729cd70d4f34f6be3d9527579bd93c672766b3fd5e637c45e54
```

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
