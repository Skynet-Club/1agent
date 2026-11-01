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
