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
| Clippy `-D warnings` | passed |
| Clean-source package gate | passed |

## Known limitations

- Limits are Unicode scalar counts, not grapheme-cluster counts.
- Existing oversized legacy candidates will fail closed on replay.
- Limits are code constants, not per-scope configuration.
- Verification evidence text and review reasons have separate existing checks.

## Verified clean release

```text
source commit: ebaa2e9ba7e94c12a8b381c632cd0c9f970cd993
git dirty: false

Linux binary:
87d5c0eedea38877d7e150a20e859d4a886e07ff4658e27b379baaa7aca1d89d

Windows binary:
69d0b2e08899e11ad10b0c9acf30d52b08c62e317bd16cd364dba1fe03860009

Linux tar.gz:
b4ced617d0a6081a4426a4e8ea958d51b06d3d5b34a6abfbb0baa2efed63dc49

Windows zip:
1f4f236ecd8c22fcb46f832d3e3133121ae53f1e22619b554f6b526485d6c910

Release manifest:
b99eb1a217047a57c594c9a1c81865000742551c8a0b3dc2962168417d277f71
```

## Next requirement

Bound and audit experience transition metadata:

1. cap actor and reason text;
2. reject invisible controls in transition metadata on replay;
3. validate event timestamps are monotonic;
4. bound experience event line size;
5. test oversized and malformed transition events.
