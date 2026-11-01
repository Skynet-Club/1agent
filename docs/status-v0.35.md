# Status v0.35.0

Date: 2026-11-02
Status: route-aware resume status and policy recovery implemented

## Changes since v0.34.0

- Added `resume-status --journal <path> [--json]`.
- Status reports:
  - route-aware versus legacy;
  - cognitive mode;
  - next step and tool count;
  - elapsed time;
  - route-plan digest;
  - policy sources;
  - effective route objective.
- Checkpoints now store the effective route objective.
- If `resume` has no objective override, it reuses the checkpoint objective.
- An explicit conflicting objective is rejected before provider execution.
- Stored budget-source attribution is preserved during resume.

## Verification

| Check | Result |
|---|---|
| Route-aware status | passed |
| Legacy classification | implemented |
| Automatic objective recovery | passed |
| Explicit conflicting override | rejected |
| Stored objective persistence | passed |
| CLI resume E2E | passed |
| Unit tests | 76 passed |
| Core evaluation rubric | 4/4 passed, 0 hard-gate failures |

## Known limitations

- Resume-status validates checkpoint structure through the existing resumable-prefix checker.
- Provider changes that leave the effective plan unchanged are still permitted.
- Legacy checkpoints report `legacy=true` and cannot be proven route-equivalent.

## Next requirement

Extend human-facing run traces with route decisions:

- show the route plan summary in non-JSON command output;
- keep provider credentials and endpoints out of traces;
- include rejected-route counts and top selected reason;
- test replay and audit output stability;
- avoid duplicating the full plan in user answer text.
