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
| Clean-source package gate | passed |

## Verified clean release

```text
source commit: 9f61c877c2a47f7fcdaf368e7ddc1fc510f03902
git dirty: false

Linux binary:
9be87ec09ef3dce0bd1c380db21aa0a06646f551d26991ac2636f576463bc35e

Windows binary:
9004de5df9cd844ec96878a7bdf42a3cd25fc6bf9700874abb61d67b5fcef115

Linux tar.gz:
8810ba56564ebb4d6f94c7d57bd0262c41b08b4c1b8ada06b322b62a7e6ebebc

Windows zip:
7171c0b5da31013396ab7d5d29cd7675d8497e15be8679bbc0f1ccf300d75e10

Release manifest:
4557c8cc8aeaa735c9e4e9ae4aab8a30f9d825c52a46bd50f30a68b1c21ec8d6
```

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
