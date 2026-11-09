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
- suite version, unique case ids and a 64-character hexadecimal SHA-256;
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
Linux binary:   2553805adac7dc25765f77486d49566c46bafb8433c8537a0680f1557de14ace
Windows binary: ef20f4d83199687bdcf6e694cb2ead633cd4ae628340738eedb8d1479c8ff37a
Linux tar.gz:   fd354eaedc86027121aaf5cfc7151649c6e365b3f074f7bf9472e0e633dc5f9a
Windows zip:    86b7d6c08d591694e8abc92e7637fe6d75e33d377d2909c0fb290ddfb21f8325
```

## Requirement comparison

| Requirement | Status |
|---|---|
| Held-out split gate | implemented |
| Negative-transfer blocker | implemented |
| Increased false-certainty blocker | implemented |
| Cost-growth gate | implemented |
| Verification evidence persistence | implemented |
| Suite metadata binding | implemented |
| Automated evidence generation | pending |
| Real-provider evaluation | blocked by missing credentials |

## Next requirement

Connect the evaluation harness to experience verification:

1. Run baseline and learned cases under the same deterministic contract.
2. Generate a verification report from measured case outcomes.
3. Prevent hand-written metrics from silently bypassing the harness.
4. Store suite version, case ids and checksums with the report.
5. Keep human approval after automated verification.
