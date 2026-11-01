# Status v0.68.0

Date: 2026-11-02
Status: experience resource bounds implemented

## Limits

```text
title:                200 Unicode scalar values
content:            4,000 Unicode scalar values
scope:                200 Unicode scalar values
evidence references:   32
applicable modes:      1..=6 unique values
```

Limits apply before proposal persistence and again during experience-store replay.

## Verification

| Check | Result |
|---|---|
| Unit tests | 114 passed |
| Integration / E2E | passed offline |
| Core evaluation | 6/6 passed |
| oversized title | rejected |
| oversized content | rejected |
| oversized scope | rejected |
| too many evidence refs | rejected |
| duplicate modes | rejected |
| empty modes | rejected |
| normal candidate | accepted |
| Clippy `-D warnings` | pending |
| Clean-source package gate | pending |

## Known limitations

- Limits are Unicode scalar counts, not grapheme-cluster counts.
- Existing oversized legacy candidates will fail closed on replay.
- Limits are code constants, not per-scope configuration.
- Verification evidence text and review reasons have separate existing checks.

## Next requirement

Bound and audit experience transition metadata:

1. cap actor and reason text;
2. reject invisible controls in transition metadata on replay;
3. validate event timestamps are monotonic;
4. bound experience event line size;
5. test oversized and malformed transition events.
