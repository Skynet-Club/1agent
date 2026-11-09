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
| Clean-source package gate | passed |

## Verified clean release

```text
source commit: e59d19e9c924c48328af0b4ba2b4a852efeb8f9e
git dirty: false

Linux binary:
687fdf0bc1efbd24a89c3d38b9fde796781c32a27ae1003c510e81e4750ab710

Windows binary:
ccb6dbb3643e510ca02b76f6d1eff82405708a904aa52aa3a3a545b3d3ce029a

Linux tar.gz:
75967c0f418b197b21f0eaf443121bdb3b249b06842ca6991c1daf0174d89556

Windows zip:
19c930509952a128179207066e08204d45a58573b8782be977034c7d763f69d6

Release manifest:
973bc6d223a7de6fc281f63870c64d593431b851d2d3dc19112ea0b49dc35724
```

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
