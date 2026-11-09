# Status v0.13.0

Date: 2026-11-02
Status: frozen gold-set manifest and reviewer schedule

## Added since v0.12.0

- `annotate create-goldset`.
- `annotate verify-goldset`.
- Manifest fields:
  - gold-set version and suite id;
  - suite SHA-256;
  - owner and reviewer cohort;
  - redaction policy;
  - freeze timestamp;
  - deterministic review batches;
  - manifest SHA-256.
- Verification detects suite drift and batch coverage errors.
- Calibration requires a verified frozen gold-set manifest.
- Calibration predictions outside the manifest case set are rejected.

## Verification

| Check | Result |
|---|---|
| Unit tests | 52 passed |
| Gold-set batch generation | passed |
| Suite drift detection | passed |
| Manifest self-digest | passed |
| Calibration gold-set gate | compiled and CLI validated |
| Core evaluation rubric | 4/4 passed |
| Clippy `-D warnings` | passed |

Release artifacts:

```text
Linux binary:   d11e0d0b5ab709e614e5b2579832ccd30c12acf1ae45ccf2be91afe977bdf3d2
Windows binary: 8c5ea43761b4ffaab3537504b5931739fe63ae76bdde2ed810e1571f0352aa75
Linux tar.gz:   ef9ce8b2b484c5062aef5b43785764ee1a5677f6f44f9ee8a4f3253686ca32bc
Windows zip:    e1f73d58d72b2e88ebdd54975fac6a16dba90c94dd245cd315b2f1c3f6115b6e
```

## Requirement comparison

| Requirement | Status |
|---|---|
| Frozen gold-set version | implemented |
| Reviewer cohort and owner | implemented |
| Redaction policy field | implemented |
| Review batches | implemented |
| Suite drift detection | implemented |
| Calibration input restriction | implemented |
| Real reviewer identities | not available/not authenticated |

## Known risks

- Redaction policy is declared, not automatically enforced against labels.
- Reviewer identity remains an unauthenticated local string.
- Manifest freezing records a timestamp but no external signature or trusted timestamp.
- Batch scheduling is generated, not connected to a notification workflow.

## Next requirement

Add an external approval/signature envelope for gold-set and experience artifacts:

1. Define a detached signature or approval record.
2. Bind approver identity, artifact digest and timestamp.
3. Verify artifact approvals before calibration or activation.
4. Keep unsigned artifacts usable for development only.
5. Document key management and local trust boundaries.
