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
| Clean-source package gate | passed |

## Verified clean release

```text
source commit: 936d0ca4c6af937fd553c4af1f16f403cdeb89d1
git dirty: false

Linux binary:
bd1bdb2e44289f0dbc045330132c61f3f736cb16e87cf87b1df4a9115f6738ee

Windows binary:
42290e061468ef68d0711f6f2041081f47e49e815221b51624796ed1a3b62548

Linux tar.gz:
495b3bf9990ffea2a85b141792559683e41464d1871b81c77efcd320a077fde7

Windows zip:
8b7b8ab41e40296f4868d2e71d70c79d2d857db5bb2f68d7834926291faaa7f4

Release manifest:
b3a2063e20db3bb9fff5f6276444b2bce00b4c6189c86bdc6ab959171c91d67e
```

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
