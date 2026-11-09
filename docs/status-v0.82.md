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
| Clean-source package gate | passed |

## Release artifacts

Source commit: `d2546c54814ef3985d718a619989b24e88580961`

Clean-source artifacts were rebuilt after the feature commit. SHA-256:

| Artifact | SHA-256 |
|---|---|
| Linux `tar.gz` | `5608ce937795d2e9961397516e5cedd022da7a438701da5aefcd3e0dded20f83` |
| Windows `zip` | `323bf6c306aa48df972e156008beb54d12df3b11dd4f12a904cf00ccb5344782` |
| Release manifest | `d924eb33c357a28dacdbbb2f2b1093ce4259964321e64401f6d00e83c849445f` |
| Linux binary | `58c4781eb5b1c5cb02524c8ef2c3f04f7545d2eddcce34c0e0a679a1cdec55c6` |
| Windows binary | `146c377e299c0ccc3b6e986f955bf5d39341d6ef7d0c1b9ae2889d271739ef71` |

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
