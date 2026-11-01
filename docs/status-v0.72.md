# Status v0.72.0

Date: 2026-11-02
Status: human-readable experience lifecycle summary implemented

## Changes since v0.71.0

Non-JSON `experience audit` output now includes:

```text
events
candidates
active
superseded
chains
depth
expired_active
```

Failures print the same summary followed by stable finding codes and messages. JSON output is
unchanged.

## Verification

| Check | Result |
|---|---|
| Unit tests | 118 passed |
| Integration / E2E | passed offline |
| Core evaluation | 6/6 passed |
| clean text summary | passed |
| active count display | passed |
| supersession metrics display | passed |
| JSON compatibility | passed |
| Clippy `-D warnings` | pending |
| Clean-source package gate | pending |

## Known limitations

- Text output is intended for humans; clients should use JSON.
- Nested findings are not grouped by category.
- Counts reflect locally replayable candidates only.
- Expired active count uses wall-clock time.

## Next requirement

Add a decision-completeness rollup to evaluation scorecards:

1. count conditional answers with bound flips;
2. count completed answers with high-impact assumptions;
3. count high-impact unknowns lacking reduction;
4. expose counts in JSON and text scorecards;
5. add a failing fixture case.
