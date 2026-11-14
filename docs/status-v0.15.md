# Status v0.15.0

Date: 2026-11-02
Status: approval key rotation and revocation

## Added since v0.14.0

- Approval key registry.
- Key states:
  - active;
  - retired;
  - revoked.
- `approval key-add`.
- `approval key-retire`.
- `approval key-revoke`.
- Retired keys can verify historical approvals but cannot sign new ones.
- Revoked keys are rejected before artifact verification.
- Calibration requires approval verification through the registry.
- Key add rejects duplicates and transition rejects changes to already revoked keys.

## Verification

| Check | Result |
|---|---|
| Unit tests | 55 passed |
| Retired-key historical verification | passed |
| Retired-key new-sign refusal | passed |
| Revoked-key rejection | passed |
| Core evaluation rubric | 4/4 passed |
| Clippy `-D warnings` | passed |

Release artifacts:

```text
Linux binary:   8f68a8131f4b901fdafd13102f1a1801b1f061f375d1d77c4638fc6b834b8b7b
Windows binary: a1d8bbeb1c7bba71ce71cd45f278993385ca81e89e225d53c91edc3fc957a06f
Linux tar.gz:   6e6d156b1240766f0fb0144c5a74afc63565cf5bae80aac473292dbc1bf362a6
Windows zip:    e8df5648d70deea707df9e8c340a13bfc6ada89cc4f6d8af4adba6e3a092a4be
```

## Requirement comparison

| Requirement | Status |
|---|---|
| Key rotation | implemented |
| Revocation state | implemented |
| Revocation reason/time | implemented |
| Calibration revocation check | implemented |
| External trusted timestamp | pending |
| Asymmetric signatures | pending |

## Known risks

- Registry is a local JSON file without authenticated update transport.
- Revocation does not retroactively rewrite approval artifacts.
- HMAC remains symmetric and environment-backed.
- No hardware-backed key or remote key service exists in V0.

## Next requirement

Add signed-key registry snapshots:

1. Bind registry state to a manifest digest.
2. Require an active administrative key to sign registry snapshots.
3. Verify snapshots before loading key state.
4. Reject rollback to an older unsigned registry version.
5. Test signed snapshot and rollback rejection.
