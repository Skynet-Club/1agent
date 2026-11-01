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
| Clean-source package gate | passed |

## Verified clean release

```text
source commit: f98a6da8ad15e45a713fa8cbc0b13d9c206eb5e6
git dirty: false

Linux binary:
5f426a2e99f3c34045353beeab48b5410705618d0b4e71337e7dd5a0337f5142

Windows binary:
b274c7b3b1624d46652669a17323ee835704842a561b6bfdd99cd80ce2b1b7ad

Linux tar.gz:
11833e0560c3e59b14a2693b656f42d753a7531a36cfa917d400e1362055daad

Windows zip:
3c865c517aedab196b0dd19d29aa18339c883c745fc0bd1a77281df93d0a5fde

Release manifest:
f931ab4fe1c0cc851d4095a1a24a6de1c3d982d4cd7c9256974c8a06863a150f
```

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
