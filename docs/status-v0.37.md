# Status v0.37.0

Date: 2026-11-02
Status: bounded human route trace implemented

## Changes since v0.36.0

- Human-readable route selection reasons are capped at 240 characters with an ellipsis.
- Route summaries include first-model-step fallback attempt count.
- Text formatting is implemented as a testable `RouteTraceSummary::render` function.
- Full untruncated route reasons remain available in journal/replay JSON.
- Endpoint, credential and prompt fields remain absent from the summary.

## Verification

| Check | Result |
|---|---|
| Reason truncation | passed |
| Full reason preserved in JSON | structural |
| Fallback attempt count | passed |
| Normal route attempt count | passed |
| Unit tests | 76 passed |
| Core evaluation rubric | 4/4 passed, 0 hard-gate failures |
| Clean-source package gate | passed |

## Verified clean release

```text
source commit: 845fcdb24409c906564dc38210ef480eefc59b4d
git dirty: false

Linux binary:
106b432f10dcca803f89613b299881dc605aa1a8579c079aaed1677038df8665

Windows binary:
6264124fe5676d7c98f9b7f18d0d3d7c3f211823215154a3874674c048ab827f

Linux tar.gz:
e1d89fa463cf1fbfdd7deb17517db982d233456bea6686431eba3dbff0f512f4

Windows zip:
a8709a1ba1d5261c1d31c73575fb5fcd7a4163c418675ba7c2d408b658136c80

Release manifest:
1b88331a736c77089262276dc917b31bc03d2b214fa61bb05f8aaa4127cb0006
```

## Known limitations

- Fallback count covers the first model step only.
- Later-step retries remain visible only in the detailed journal.
- Terminal width is fixed rather than detected.

## Next requirement

Improve provider output normalization across OpenAI-compatible implementations:

1. Support content arrays where the API returns multiple text parts.
2. Normalize missing tool-call IDs deterministically.
3. Reject empty or malformed tool names.
4. Keep provider-specific response types out of the domain.
5. Add adapter conformance tests for each normalization rule.
