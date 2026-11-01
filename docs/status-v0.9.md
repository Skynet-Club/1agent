# Status v0.9.0

Date: 2026-11-01
Status: held-out experience verification and journal-bound host lifecycle

## Experience verification

Candidate lifecycle is now:

```text
draft -> reviewed -> verified -> approved -> active
                   \-> rejected
active -> degraded -> rolled_back
```

Verification requires:

- distinct training and evaluation groups;
- positive held-out success improvement;
- no increase in unsupported-certainty rate;
- cost increase no greater than 10%;
- non-empty evidence reference;
- all rates between 0 and 10000 basis points.

Approval is rejected unless the candidate is `verified`.

## Host lifecycle

- External host request ids are persisted as `ClientRequestLinked` journal events.
- Host status returns the authoritative journal path for terminal requests.
- `history` reconstructs run and client request ids using bounded head/tail reads.
- Recent lifecycle memory is bounded to 256 requests and remains non-authoritative.

## Verification

| Check | Result |
|---|---|
| Unit tests | 45 passed |
| Experience verification CLI | passed |
| Leakage and false-certainty rejection | passed |
| Host request-id persistence | passed |
| Journal history request-id recovery | passed |
| Core evaluation | 4/4 passed |
| Clippy `-D warnings` | passed |

Release artifacts:

```text
Linux binary:   4c86756889b24611aef50d92ab74b2c178e3eb48a793c607a7ccfdd09c100037
Windows binary: 15b6bee2403d5d669b375b9b234135d393364ea12d4f202ddcfd1db749e88c59
Linux tar.gz:   9fc46d7c4a389f96e0757f2ad0d68c7caca01fbf956c60c1d703d8bd2b8a06d2
Windows zip:    e462fb081a6675bfa5120b87739d332412f1aaa45237fd8eed03a44cac7dcc73
```

## Requirement comparison

| Requirement | Status |
|---|---|
| Held-out split gate | implemented |
| Negative-transfer blocker | implemented |
| Increased false-certainty blocker | implemented |
| Cost-growth gate | implemented |
| Verification evidence persistence | implemented |
| Automated evidence generation | pending |
| Real-provider evaluation | blocked by missing credentials |

## Next requirement

Connect the evaluation harness to experience verification:

1. Run baseline and learned cases under the same deterministic contract.
2. Generate a verification report from measured case outcomes.
3. Prevent hand-written metrics from silently bypassing the harness.
4. Store suite version, case ids and checksums with the report.
5. Keep human approval after automated verification.
