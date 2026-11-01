# Status v0.82.0

Date: 2026-11-02
Status: explicit report verification status implemented

## Verification metadata

Compact summaries now include:

```json
{
  "verification": {
    "integrity_checked": true,
    "content_binding": "bound",
    "authenticity": "unauthenticated"
  }
}
```

- `integrity_checked` means report policy, arithmetic and aggregates passed inspection.
- `content_binding` is `bound` when `report_content_sha256` is present, otherwise `unbound`.
- `authenticity` is `unauthenticated`; a content hash is not a signature.
- Summary schema version `3` requires current verification metadata.
- Versions `0..=2` retain their legacy unbound/unchecked interpretation.

## Verification

| Check | Result |
|---|---|
| Unit tests | 123 passed |
| Integration / E2E | passed offline |
| Core evaluation | 6/6 passed |
| integrity status | passed |
| bound digest status | passed |
| unauthenticated status | passed |
| missing current verification | rejected |
| mismatched binding status | rejected |
| legacy summary | accepted |
| Clippy `-D warnings` | passed |
| Clean-source package gate | pending |

## Known limitations

- The current implementation cannot issue signed report attestations.
- Summary consumers must still decide whether unauthenticated evidence is acceptable.
- Integrity checks prove internal consistency, not external truth.
- Digest binding detects accidental or unsophisticated mutation, not malicious rewriting.

## Next requirement

Add an explicit trust decision contract for hosts:

1. accept minimum report trust requirements;
2. reject unauthenticated reports when a host requires signatures;
3. preserve current informational inspection;
4. test default, required-integrity and required-authenticity policies.
