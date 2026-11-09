# Status v0.10.0

Date: 2026-11-02
Status: automated held-out experience evaluation

## Added since v0.9.0

- `experience evaluate` command.
- Same held-out suite is run twice:
  - baseline: no candidate injected;
  - learned: reviewed candidate injected as shadow policy.
- Candidate is not activated before evaluation.
- Metrics are generated from actual case outcomes:
  - success basis points;
  - unsupported-certainty basis points;
  - duration delta as the initial cost proxy.
- Report binds suite id/version, sorted case ids and SHA-256 of the actual suite bytes.
- Passing evaluation writes the candidate into `verified`; failing evaluation remains rejected from
  transition.
- Human approval is still required after automated verification.
- Eval cases can define a human-reviewed rubric for:
  - required and forbidden conclusion terms;
  - required evidence sources;
  - required claim statuses.
- Eval reports include rubric passed/check totals.

## Verification

| Check | Result |
|---|---|
| Unit tests | 46 passed |
| Positive learned-transfer fixture | passed |
| Baseline failure vs shadow success | passed |
| Automatic suite SHA/case-id report | passed |
| Core rubric | 4/4 checks passed |
| Existing held-out and experience lifecycle tests | passed |
| Clippy `-D warnings` | passed |
| Core evaluation | 4/4 passed |

Release artifacts:

```text
Linux binary:   73a7ae294dd68715497fe556cc3b202b7db2f5dc4b4eb48edc4014f2b0fffd1a
Windows binary: 7c484aac2590dde73c52f6b3e84868e60319b1372f17ce1587a7fa7a176bef3d
Linux tar.gz:   6fdcdf31e7b0fc8b303d078fa358a98326e9b557f07f0f611f42b43900645473
Windows zip:    146fb7e7d62187ddc000e51a3bcce8ac675e147c1d071c0c62db6d3286ebb990
```

## Requirement comparison

| Requirement | Status |
|---|---|
| Automated baseline run | implemented |
| Shadow candidate evaluation | implemented |
| Automated report generation | implemented |
| Suite hash/case binding | implemented |
| Human approval after verification | implemented |
| Human-reviewed evidence rubric | implemented |
| Production model quality | unverified, no credentials |
| Monetary cost delta | partial; duration proxy used until provider pricing is configured |

## Next requirement

Add calibrated confidence after accumulating human labels:

1. Accumulate more redacted human-gold fixtures.
2. Measure inter-rater agreement before automating judgment.
3. Compute calibration and selective-risk curves when labels justify it.
4. Expose numeric confidence only if calibration passes.
5. Keep executable contract tests as the release gate.
