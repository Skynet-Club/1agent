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
| Clippy `-D warnings` | passed |
| Clean-source package gate | passed |

## Known limitations

- Text output is intended for humans; clients should use JSON.
- Nested findings are not grouped by category.
- Counts reflect locally replayable candidates only.
- Expired active count uses wall-clock time.

## Verified clean release

```text
source commit: c4252573886ed21dc38d44ccab3f2090ecabef6a
git dirty: false

Linux binary:
4f9ec3ab319843ee0cca5716a892730e1764f337cd65a9513b8f3baebff40132

Windows binary:
60e35b5fb4306bc2cc2c1983755ca2ea04604e7f1b758f36dea01bc43ede5900

Linux tar.gz:
b218cc43d180e9bf0c4c2320372dfca78096faf60419c55db20f34cd039362e2

Windows zip:
5a9eab9bdea39ff872ad73cd190f340b1949d7a891543b9b6b94a775cee57274

Release manifest:
311ac448e77f368a1097d69af5b09a473f0f56dfc55ddcacff94488793206b68
```

## Next requirement

Add a decision-completeness rollup to evaluation scorecards:

1. count conditional answers with bound flips;
2. count completed answers with high-impact assumptions;
3. count high-impact unknowns lacking reduction;
4. expose counts in JSON and text scorecards;
5. add a failing fixture case.
