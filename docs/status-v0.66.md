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
| Clippy `-D warnings` | pending |
| Clean-source package gate | pending |

## Known limitations

- `provider_failed` is allowed for all experience kinds rather than route-policy only.
- `model_responded` is not considered verified evidence.
- Evidence content quality still requires review and held-out evaluation.
- The class list is a code contract, not a separately versioned registry.

## Next requirement

Audit experience proposal evidence classes and digest metadata at store replay:

1. reject unknown evidence classes when replaying candidates;
2. verify digest format for new candidates;
3. preserve legacy digest-free entries;
4. detect unsupported experience schema versions;
5. test corrupted experience event history.
