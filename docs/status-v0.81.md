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
| Clean-source package gate | passed |

## Release artifacts

Source commit: `1299e14f6dd653dce42b279c8d81cb0ec9fb83f8`

Clean-source artifacts were rebuilt after the feature commit. SHA-256:

| Artifact | SHA-256 |
|---|---|
| Linux `tar.gz` | `36a3525f73a2854516d7722023e311a0a74d712978475259af7ede6f63eb6660` |
| Windows `zip` | `37093b40e844c5cf7fdd6ed9e9fadf0a4a8bd7adba402e37bad55f22ae28199f` |
| Release manifest | `5c211ce27fe1f7b687b9cc90ee00875aaac3cbe9cbf404d38740f2d22499ea47` |
| Linux binary | `658b1d870f1d799022e5c9cdde390a9384ee455659304a64e4e6fca9144086ff` |
| Windows binary | `84181a75ea0db0fb581452b554eaf049c9e2015510f8a8b1f4044f6464b8e991` |

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
