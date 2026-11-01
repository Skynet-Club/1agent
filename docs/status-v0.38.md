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
| Clean-source package gate | passed |

## Verified clean release

```text
source commit: efb600573a987cbb216bfe7c5afdfeb4ededc9b8
git dirty: false

Linux binary:
0a352438ca981d609e298f313822fd1e4ee548b8b0cc56827d7cd85debf9dbe9

Windows binary:
c7ae723e7e4cdb234f9d58a2fe067adb984e07fae8480695d4831c74f8e9f21a

Linux tar.gz:
4cc8dca66221d1ff944c780505c48f4ba0fef1e8907a7571b7702371b73bc108

Windows zip:
d5980ae91ce7dce0d15f7e1de11364ef39cc3cb99e52bfb7997cdf2e6f85ce2d

Release manifest:
7f7cc5e7dbadba5d82f35b5e92c107cc8fb16020995e3f8a746425bd9b8d0343
```

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
