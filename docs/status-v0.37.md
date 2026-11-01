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
