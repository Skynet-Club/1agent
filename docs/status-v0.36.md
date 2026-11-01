# Status v0.36.0

Date: 2026-11-02
Status: human-readable route trace implemented

## Changes since v0.35.0

- Non-JSON `run` and `resume` output now appends:
  - cognitive mode;
  - route objective;
  - ranked candidate count;
  - rejected candidate count;
  - first actual provider/model selection;
  - selection reason;
  - policy-source attribution.
- Trace rendering reads the authoritative journal after completion.
- Full route plans remain available through journal/replay JSON.
- Route summaries omit endpoints, credentials and prompt content.

## Verification

| Check | Result |
|---|---|
| Route trace summary | passed |
| Selected route shown | passed |
| Policy sources shown | passed |
| JSON output unchanged | preserved |
| Secret/endpoint omission | structural |
| Unit tests | 76 passed |
| Core evaluation rubric | 4/4 passed, 0 hard-gate failures |

## Known limitations

- Only the first selected route is shown in human output.
- Fallback attempts remain in the detailed journal.
- Text labels use Rust debug formatting for enums and may be localized later.
- Very long reason strings are not truncated in terminal output.

## Next requirement

Add bounded trace rendering:

1. Cap human-readable reason lengths.
2. Show fallback attempt count without repeating the full plan.
3. Add stable text-output tests.
4. Keep full details in JSON.
5. Avoid printing provider endpoint information.
