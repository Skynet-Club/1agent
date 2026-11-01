# Status v0.50.0

Date: 2026-11-02
Status: explicit contradiction records and resolution state implemented

## Changes since v0.49.0

- Added conflict records with:
  - unique ID;
  - exactly two distinct claim side IDs;
  - unresolved or resolved status;
  - optional resolution text;
  - resolution sources.
- Resolved conflicts require both resolution text and at least one source.
- Resolved conflicts cannot retain conflicting side claims.
- A claim marked `conflicting` must appear in an unresolved conflict record.
- Unresolved conflicts must not carry resolution evidence.
- Evaluation adds `min_unresolved_conflicts`, conflict count and unresolved count.
- Provider schema and scripted conflict fixture include conflict records.

## Verification

| Check | Result |
|---|---|
| Two-side unresolved conflict | passed |
| Resolved without evidence | rejected |
| Resolved with conflicting claim | rejected |
| Unresolved conflict count | passed |
| Scripted fixture | 3/3 passed |
| Core evaluation | 6/6 passed |
| Unit tests | 87 passed |

## Known limitations

- Two sides are required; multi-party conflicts need multiple records.
- Resolution quality is reviewed externally, not inferred.
- Conflict IDs are local to an answer.

## Next requirement

Bind conflicts to evidence provenance:

1. Require resolution sources to be declared in involved claims or a resolution claim.
2. Add source freshness/timestamp metadata.
3. Score unresolved conflict impact.
4. Require high-impact conflicts to have a reduction plan.
5. Test source-bound resolution.
