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
| Clippy `-D warnings` | pending |
| Clean-source package gate | pending |

## Known limitations

- `BufRead::lines` still allocates a malformed oversized line before rejection.
- Equal timestamps are accepted because millisecond resolution can collide.
- The experience file has no global hash chain.
- Limits are code constants.

## Next requirement

Add machine-readable experience-store audit status:

1. report invalid sequence, timestamps, digest and transition reasons;
2. verify the terminal state of every supersession chain;
3. report active uniqueness violations;
4. keep candidate listing functional for valid stores;
5. add JSON audit CLI coverage.
