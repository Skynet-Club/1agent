# Status v0.69.0

Date: 2026-11-02
Status: experience transition metadata bounded

## Changes since v0.68.0

- Actor maximum: 128 Unicode scalar values.
- Reason maximum: 1,000 Unicode scalar values.
- Verification evidence maximum: 1,000 Unicode scalar values.
- Experience event line maximum: 64 KiB.
- Event timestamps must be non-decreasing.
- Hidden controls in transition metadata are rejected during replay.
- Oversized new events fail before append.

## Verification

| Check | Result |
|---|---|
| Unit tests | 115 passed |
| Integration / E2E | passed offline |
| Core evaluation | 6/6 passed |
| oversized actor replay | rejected |
| non-monotonic timestamp | rejected |
| oversized event line | rejected |
| hidden controls | rejected |
| normal history | passed |
| Clippy `-D warnings` | passed |
| Clean-source package gate | passed |

## Known limitations

- `BufRead::lines` still allocates a malformed oversized line before rejection.
- Equal timestamps are accepted because millisecond resolution can collide.
- The experience file has no global hash chain.
- Limits are code constants.

## Verified clean release

```text
source commit: 78f35387bdd103b559a44ba8b3ef2f856ed1d7bf
git dirty: false

Linux binary:
d1028605bed0efacbb2b31c32b0d5252a32a50d2aaf1469ca61a55e5bfe161bc

Windows binary:
94d0f2148d2b4ad6020671e9c9f8c0210265dcde1b5a44027153a2d2fb069169

Linux tar.gz:
dfac4a6e78415213cf1e29bbd679d864691b21ef2d68bb763a4181b6063c0459

Windows zip:
f19357b19947d26581ebcf0f006a4fff444cc4805da4849308397bc9cf8d9522

Release manifest:
bb2a8ff70ae9d0d085cfa26ef55c89760a8096d2ec17c888b2817c31f26c3d3b
```

## Next requirement

Add machine-readable experience-store audit status:

1. report invalid sequence, timestamps, digest and transition reasons;
2. verify the terminal state of every supersession chain;
3. report active uniqueness violations;
4. keep candidate listing functional for valid stores;
5. add JSON audit CLI coverage.
