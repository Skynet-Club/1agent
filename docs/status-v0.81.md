# Status v0.81.0

Date: 2026-11-02
Status: content-bound evaluation reports implemented

## Report digest

Generated reports include `content_sha256`. The digest covers the normalized report structure
with its own digest field set to null. Compact summaries expose the same value as
`report_content_sha256`.

Summary schema version `2` adds digest binding. Versions `0` and `1` remain readable and are
treated as unbound documents.

## Verification

| Check | Result |
|---|---|
| Unit tests | 123 passed |
| Integration / E2E | passed offline |
| Core evaluation | 6/6 passed |
| generated report digest | passed |
| summary digest binding | passed |
| changed case payload | rejected |
| legacy report without digest | accepted |
| legacy summary without digest | accepted |
| Clippy `-D warnings` | passed |
| Clean-source package gate | pending |

## Known limitations

- SHA-256 detects content changes but does not authenticate the producer.
- A party that rewrites the payload can recompute the digest.
- Digest stability depends on the canonical struct serialization contract.
- Legacy unbound reports remain structurally valid.

## Next requirement

Add a verification summary that distinguishes integrity, content binding and authenticity:

1. report digest status;
2. policy compatibility status;
3. digital signature status;
4. keep unauthenticated reports explicitly unauthenticated.
