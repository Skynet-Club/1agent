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

## Verification

| Check | Result |
|---|---|
| Unit tests | 46 passed |
| Positive learned-transfer fixture | passed |
| Baseline failure vs shadow success | passed |
| Automatic suite SHA/case-id report | passed |
| Existing held-out and experience lifecycle tests | passed |
| Clippy `-D warnings` | passed |
| Core evaluation | 4/4 passed |

Release artifacts:

```text
Linux binary:   908ef7bc6a55b864c421ebda2adcbb4b85c24ce66ae366005fcd83799b5165e1
Windows binary: 5fdef749a28b5920ff244190550333c36e9a349ab3805ed691fdda02d85fcc36
Linux tar.gz:   58823ae8587cdebb02ac28bbded608a7903f45c493e232d555ad06add9aebe9a
Windows zip:    a94d99209b66c2865efbfa61eb9038127bde6c3c1811471f11fe06a7abb44855
```

## Requirement comparison

| Requirement | Status |
|---|---|
| Automated baseline run | implemented |
| Shadow candidate evaluation | implemented |
| Automated report generation | implemented |
| Suite hash/case binding | implemented |
| Human approval after verification | implemented |
| Production model quality | unverified, no credentials |
| Monetary cost delta | partial; duration proxy used until provider pricing is configured |

## Next requirement

Add calibrated outcome and evidence scoring for open-ended cases:

1. Add redacted human-gold fixtures.
2. Score citation support separately from answer style.
3. Measure abstention and conditional-answer quality.
4. Calibrate optional confidence before exposing numeric confidence.
5. Keep executable contract tests as the release gate.
