# Status v0.47.0

Date: 2026-11-02
Status: strict claim/evidence provenance invariants implemented

## Rules

### Claim status

- `verified`: at least one non-empty source.
- `derived` and `assumed`: non-empty scope.
- `model_prior`: no external evidence source.
- `conflicting`: at least two distinct sources.

### Evidence

- Source and statement must be non-empty.
- At least one claim reference is required.
- Claim references within one evidence record must be unique.
- Referenced claim must declare the evidence source.

## Enforcement

These checks run in `AgentAnswer::validate` and therefore apply to runtime answers, journal audits,
evaluation cases and provider fixture outputs.

## Verification

| Check | Result |
|---|---|
| Verified source requirement | passed |
| Assumed/derived scope requirement | passed |
| Model-prior source rejection | passed |
| Conflict source minimum | passed |
| Duplicate evidence reference | rejected |
| Undeclared evidence source | rejected |
| Unit tests | 84 passed |
| Core rubric | 6/6 passed |
| Clean-source package gate | passed |

## Verified clean release

```text
source commit: aef4182fb7e1763644f73972def9adb12c6884a2
git dirty: false

Linux binary:
bf3b85a58c1b64d8c8e85ddab3e71ecf5bf480a5e236430a8de8ee8a1c2f7152

Windows binary:
9ab8dee528086420c3f33094cd82d83e7387033a1b5b6c7bfed8113fc74ae094

Linux tar.gz:
84f642f4907d11b725dae0f6611310b1549a9eaf62d785ec5d2f5988278a4b87

Windows zip:
c6c543e7f9c042e28e498ec099512443b0596d700adeebfc67b607618b88ded2

Release manifest:
c04e57d7177573094f07c24ffd1062e22837fde5d1251b02ef036481100bad5e
```

## Known limitations

- Source IDs are not yet resolved to a global evidence registry.
- This validates structure, not whether a cited document supports the statement.
- Human review remains required for semantic support and source authority.

## Next requirement

Introduce explicit unknown/assumption impact ranking:

1. Add `impact` to unknowns and assumptions.
2. Require high-impact unknowns to have a reduction method.
3. Add scorecard reporting by impact.
4. Keep free text for explanation.
5. Test serialization and evaluation gates.
