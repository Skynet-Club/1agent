# Status v0.49.0

Date: 2026-11-02
Status: structured assumptions with compatibility implemented

## Changes since v0.48.0

- `AgentAnswer.assumptions` now contains:
  - description;
  - impact;
  - optional verification method.
- Legacy string assumptions decode as medium-impact records.
- High-impact assumptions require a non-empty verification method.
- Evaluation expectations add `min_high_impact_assumptions`.
- JSON scorecards report low, medium and high assumption counts.
- Human-readable answers show assumptions with impact.
- Provider system prompt documents the structured assumption shape.

## Verification

| Check | Result |
|---|---|
| Legacy string decoding | passed |
| Structured encoding | passed |
| High impact with verification | passed |
| High impact without verification | rejected |
| Impact counts | passed |
| Unit tests | 86 passed |
| Core eval | 6/6 passed |

## Known limitations

- Assumption impact is authored, not calibrated.
- Legacy string re-serializes as structured JSON after decoding.
- Verification method existence does not prove that verification occurred.

## Next requirement

Add explicit contradiction handling:

1. Require conflict records to identify both sides.
2. Add conflict-resolution status.
3. Prevent resolved contradictions from remaining marked conflicting.
4. Add scorecard gates for unresolved conflicts.
5. Keep conflict evidence source-bound.
