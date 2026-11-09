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
| Clean-source package gate | passed |

## Verified clean release

```text
source commit: 843474cea666d881e89dac0b3e903cdc3768729f
git dirty: false

Linux binary:
d8188f6d1e8b56b161d422cdc11c5239997d9ed3044afddc4c5fa9077753597d

Windows binary:
b1e6ce252c1c7e12e6953ad64e888188d2d228f412e182b056e833f36b93ea14

Linux tar.gz:
45ad525611bb23c6f27cfe064b78777674837cb1f3500599fa6c74935b344640

Windows zip:
44b484151f0d5232af9fed82039eaaef3b5b7753454bbbffcf492f6dbe6b63d8

Release manifest:
ebd9e469d7024b66989582089f753bdbcca92cdd78748fb04f7936cbdc06fca4
```

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
