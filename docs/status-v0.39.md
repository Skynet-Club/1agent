# Status v0.39.0

Date: 2026-11-02
Status: bounded tool argument schema validation implemented

## Changes since v0.38.0

- Added a central argument validator before `AgentTool::execute`.
- Supported JSON Schema subset:
  - `type`: object, string, integer, boolean, array;
  - object `required`;
  - `additionalProperties: false`;
  - nested object properties;
  - array `items`;
  - string `minLength` / `maxLength`;
  - integer `minimum` / `maximum`;
  - `enum`.
- Invalid arguments produce `invalid_arguments` without invoking the tool.
- Tool-specific path containment, permission, side-effect, timeout and concurrency checks remain
  mandatory.
- Added nested schema and unexpected-field tests.

## Verification

| Check | Result |
|---|---|
| Valid nested arguments | passed |
| Required/type/bounds/enum | covered |
| Unexpected field | rejected before tool |
| Built-in tool behavior | retained |
| Unit tests | 79 passed |
| Core evaluation rubric | 4/4 passed, 0 hard-gate failures |

## Known limitations

- This is intentionally not a complete JSON Schema implementation.
- `oneOf`, `anyOf`, `$ref`, conditional schemas and custom formats are unsupported.
- Schema validation cannot replace semantic/path/permission checks.
- Provider tool arguments can still be malformed JSON; parsing happens before this gate.

## Next requirement

Improve external tool observations:

1. Bound all observation summary/content sizes.
2. Preserve truncation metadata.
3. Reject hidden control characters centrally.
4. Keep structured status and error codes.
5. Add adversarial oversized-observation tests.
