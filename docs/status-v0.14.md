# Status v0.14.0

Date: 2026-11-02
Status: externally approved artifacts

## Added since v0.13.0

- Detached `ApprovalEnvelope`.
- Artifact kinds:
  - gold set;
  - experience bundle;
  - evaluation report.
- Approval binds:
  - artifact SHA-256;
  - approver;
  - purpose;
  - approval time;
  - key id;
  - HMAC-SHA256 MAC.
- `approval sign` and `approval verify`.
- Secret key is read from a named environment variable and never printed.
- Key length below 16 bytes is rejected.
- Artifact changed after approval is rejected.
- MAC tampering is rejected in constant-time verification.
- Calibration requires a valid signed gold-set approval.

## Verification

| Check | Result |
|---|---|
| Unit tests | 53 passed |
| Tampered artifact rejection | passed |
| Tampered approval rejection | passed |
| Calibration approval gate | compiled and CLI validated |
| Core evaluation rubric | 4/4 passed |
| Clippy `-D warnings` | passed |

Release artifacts:

```text
Linux binary:   864b0a19e41651a4e95acf27e079c305cc567e4f2995c413069d4e84716d9443
Windows binary: c1f6869b64f5262749df2364b7057a1020dfd51fbcda0cc5792363b62e456d4f
Linux tar.gz:   35dd4bd99b7a59b6fb4b8466d86f848cb0cc74241eff268ff63d0bda0f776883
Windows zip:    10b407f68c762f31338494ebab244f0dd00e0f752f24fe3b02951306750011a6
```

## Requirement comparison

| Requirement | Status |
|---|---|
| Detached approval | implemented |
| Artifact digest binding | implemented |
| Approver/purpose/time/key id | implemented |
| MAC verification | implemented |
| Signed gold-set required for calibration | implemented |
| Asymmetric signatures | not implemented; shared-key HMAC used |
| Trusted timestamp/key rotation | pending |

## Known risks

- HMAC is symmetric: anyone who can verify can also sign.
- Environment variables are the only key source in V0.
- There is no external trusted timestamp, revocation list or hardware-backed key.
- Experience activation still relies on internal approval events rather than signed bundles.

## Next requirement

Add key rotation and approval revocation:

1. Support active and retired key ids.
2. Record revocation reason and effective time.
3. Refuse revoked approvals before calibration/activation.
4. Keep verification deterministic across key rotations.
5. Add rotation/revocation tests.
