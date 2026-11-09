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
| Clean-source package gate | passed |

## Verified clean release

```text
source commit: 74e46aa6b6a603792e85506dc8c5d6f299756bd8
git dirty: false

Linux binary:
2455209664cf93cf9b897431ac94dd3666677480204bdb679c0c121f224d3334

Windows binary:
98a7b67e934d6e8561711404de99f12fabbd816719ffcf350893b5c17200ac94

Linux tar.gz:
6d7c3aff11b053cd2d761b1781f68e1583bf6ada9a8d8fda85bf1a30663e3c26

Windows zip:
1f1398a44773aaee3b50d43a3e6a8034d81977c24030700e15b21bdca75bf963

Release manifest:
41d40c51b9b812d585bf7dc63089c28b34571768fa74efa25d68df86bc251748
```

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
