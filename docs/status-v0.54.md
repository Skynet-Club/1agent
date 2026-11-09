# Status v0.54.0

Date: 2026-11-02
Status: decision completeness implemented

## Changes since v0.53.0

- Current structured answers use `schema_version: 2`.
- Version 1 remains readable and validates under its original semantic contract.
- Unknowns in version 2 have non-empty, unique IDs.
- A conditional answer must include at least one flip condition.
- A completed answer cannot depend on a high-impact assumption.
- Every flip condition must have an observation, consequence and target.
- Flip targets must be declared claims or unknowns; empty, duplicate and dangling targets
  fail closed.
- Runtime fallbacks, deterministic provider output, evaluation fixtures and provider prompts
  use the v2 contract.

## Verification

| Check | Result |
|---|---|
| Unit tests | 94 passed |
| Integration / E2E | passed |
| Core evaluation | 6/6 passed |
| Legacy v1 conditional answer | accepted |
| v2 conditional without flip | rejected |
| v2 completed with high-impact assumption | rejected |
| Dangling flip target | rejected |
| Valid unknown-bound flip | accepted |
| Journal audit of incomplete v2 answer | rejected |
| Clippy `-D warnings` | passed |
| Clean-source package gate | passed |

## Compatibility

Unversioned legacy JSON defaults to v1. New in-memory output always carries v2. Version 1 does
not acquire v2 decision-completeness requirements retroactively; this keeps historical journals
auditable under the semantics with which they were created.

## Known limitations

- Flip targets reference claims and unknowns only; assumptions and evidence need separate stable
  IDs before they can be referenced directly.
- The validator checks referential integrity, not whether a human believes the flip is plausible.
- Historical v1 journals are read-only in practice; no migration rewrites them to v2.

## Verified clean release

```text
source commit: da7019e4b3908e278ebd23092f492018a6a00687
git dirty: false

Linux binary:
eacdd729b706aaa14a135a817822b765e7866ae3f6726e89dba02782fd40f9e2

Windows binary:
57bd7fd6402d443f28e736c134c17e9817a7b62113fdbbd152f52b9153744b19

Linux tar.gz:
b955d834aa92fe0334d04fc6d612d55d25bf4e4ec356de2cab9fd8b246b9a33f

Windows zip:
4b5a564d0da9173ba74fbf75b4c1dbd22b207b065a21b1d8aa9fb80f057baa22

Release manifest:
e984a645a08cc6492e5844b1edc722b5f1b32162208d4a184b7c7b06549591da
```

## Next requirement

Make audit findings machine-readable without breaking existing text output:

1. Add stable finding codes and event indexes.
2. Retain the existing issue strings for compatibility.
3. Expose structured findings through journal audit JSON.
4. Cover duplicate answers, unsupported versions, incomplete decisions and unpaired tools.
