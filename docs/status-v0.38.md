# Status v0.38.0

Date: 2026-11-02
Status: OpenAI-compatible response normalization implemented

## Changes since v0.37.0

- Assistant `content` now accepts:
  - a plain string;
  - an array of `{ "type": "text", "text": "..." }` parts.
- Text parts are concatenated in provider order.
- Non-text or missing-text parts are rejected instead of silently dropped.
- Missing tool-call IDs receive deterministic `provider-call-<index>` IDs.
- Tool names are trimmed and restricted to ASCII alphanumeric plus `_`, `-`, `.`.
- Empty, overlong or malformed tool names are rejected.
- Duplicate tool-call IDs are rejected.
- Tool argument JSON parsing remains a mandatory adapter step.

## Verification

| Check | Result |
|---|---|
| Plain string content | passed |
| Text-part array content | passed |
| Unsupported content part | rejected by implementation |
| Missing call ID normalization | passed |
| Invalid tool name | rejected |
| Duplicate call IDs | rejected by implementation |
| Unit tests | 78 passed |
| Core evaluation rubric | 4/4 passed, 0 hard-gate failures |

## Known limitations

- Only text content parts are supported in chat completions.
- Deterministic generated IDs are local to a single provider response.
- Arguments JSON is parsed but not schema-validated until tool execution.
- The managed sandbox still blocks successful live loopback adapter tests.

## Next requirement

Improve tool argument handling:

1. Validate tool arguments against declared JSON schema before execution.
2. Return structured schema errors.
3. Keep custom concurrency, path and side-effect checks in tool code.
4. Test nested/type/enum/required constraints used by built-in tools.
5. Avoid implementing a full JSON Schema engine if a bounded subset is sufficient.
