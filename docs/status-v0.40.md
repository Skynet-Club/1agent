# Status v0.40.0

Date: 2026-11-02
Status: bounded and control-safe tool observations implemented

## Changes since v0.39.0

- Tool observations are bounded at the registry boundary:
  - summary: 4,096 characters;
  - content: 1,000,000 characters.
- Truncated values receive an ellipsis and explicit metadata:
  - `summary_truncated`;
  - `content_truncated`;
  - general `truncated=true`.
- Retained summary/content containing hidden or bidirectional controls cause the observation to
  fail with `unsafe_content`; unsafe content is not propagated.
- Structured observation status and error codes remain unchanged.
- Added adversarial custom-tool tests for oversized and unsafe observations.

## Verification

| Check | Result |
|---|---|
| Oversized summary bound | passed |
| Oversized content bound | passed |
| Truncation metadata | passed |
| Hidden control rejection | passed |
| Unsafe content not propagated | passed |
| Unit tests | 80 passed |
| Core evaluation rubric | 4/4 passed, 0 hard-gate failures |

## Known limitations

- Custom tools can still consume resources before returning an outcome.
- The content bound is character-based rather than byte-budgeted.
- Truncation may cut semantic structure such as JSON arrays/objects, so tools with structured
  payloads should pre-limit themselves.
- External process/network tools remain future work.

## Next requirement

Add host-visible tool capability status:

1. Report read-only tool names and schemas through `tools` command.
2. Distinguish implemented, degraded and unsupported platform capabilities.
3. Avoid exposing internal paths or secrets.
4. Use the same registry/spec source as runtime execution.
5. Add JSON and stable text output tests.
