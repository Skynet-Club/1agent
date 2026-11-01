# Status v0.66.0

Date: 2026-11-02
Status: experience evidence classes restricted

## Changes since v0.65.0

Allowed evidence event classes:

```text
tool_observed
answer_produced
provider_failed
```

Rejected examples:

```text
run_started
route_planned
route_selected
model_requested
checkpoint
run_stopped
```

The rule applies before digest calculation and proposal persistence.

## Verification

| Check | Result |
|---|---|
| Unit tests | 112 passed |
| Integration / E2E | passed offline |
| Core evaluation | 6/6 passed |
| tool observation evidence | accepted |
| answer evidence | accepted |
| lifecycle-only evidence | rejected |
| evidence digest remains deterministic | passed |
| CLI dynamic evidence sequence | passed |
| Clippy `-D warnings` | passed |
| Clean-source package gate | passed |

## Known limitations

- `provider_failed` is allowed for all experience kinds rather than route-policy only.
- `model_responded` is not considered verified evidence.
- Evidence content quality still requires review and held-out evaluation.
- The class list is a code contract, not a separately versioned registry.

## Verified clean release

```text
source commit: 2c9a2facf788c3fbf144d65df270c254abe11646
git dirty: false

Linux binary:
c5930e07f0cf95531e04e8619823cae723a54cdff5bceb5c39eab9f6201852bf

Windows binary:
d6ee02aa5a71d7141e3acac0728d979564e7f5dbc07a25e636b1fc09c7e935e3

Linux tar.gz:
cd92d0d5ee532d4b40e4c08cf6bf2318080dcf1c555efed0552b856f7aa7530a

Windows zip:
0302b27ab75264ee94d9ea474db6b603ef562c818410974264bcbadce304b051

Release manifest:
8d159377d2f3d32d81f8080301a0f3b495250da1398f53262fa418e30911af6e
```

## Next requirement

Audit experience proposal evidence classes and digest metadata at store replay:

1. reject unknown evidence classes when replaying candidates;
2. verify digest format for new candidates;
3. preserve legacy digest-free entries;
4. detect unsupported experience schema versions;
5. test corrupted experience event history.
