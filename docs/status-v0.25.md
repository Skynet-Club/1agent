# Status v0.25.0

Date: 2026-11-02
Status: explicit development/production release policy implemented

## Changes since v0.24.0

- `release verify` labels unsigned checks as `development policy`.
- `--production` requires all of the following:
  - release manifest approval envelope;
  - approval signing key environment variable;
  - approval key registry;
  - registry signing key environment variable;
  - at least one registry snapshot;
  - external trust anchor;
  - minimum registry version.
- Production verifies the manifest, approval, anchored snapshot chain and current registry state.
- Production-only chain options are rejected when `--production` is absent.
- The manifest file and every artifact are verified again after approval and chain checks to
  narrow check-to-use replacement windows.
- A registry state change correctly invalidates an older snapshot; a new chained snapshot is
  required.

## Verification

| Check | Result |
|---|---|
| Development manifest verification | passed |
| Production unsigned release rejection | passed |
| Production anchored snapshot verification | passed |
| Retire followed by a new chained snapshot | passed |
| Retired key validates historical signed release | passed |
| Stale pre-retirement snapshot rejection | passed |
| Unit tests | 66 passed |
| Core evaluation rubric | 4/4 passed, 0 hard-gate failures |

## Actual CLI scenario

The verification used ephemeral test keys and a real registry mutation sequence:

```text
active registry + snapshot 1 + trust anchor:
  release manifest verified (production policy)

missing signature with --production:
  rejected

retire release key + sign snapshot 2 with another active admin key:
  release manifest verified (production policy)
```

This proves policy mechanics, not production key custody.

## Known limitations

- HMAC remains symmetric.
- The trust anchor is loaded from a file; its distribution is outside the runtime.
- The default local package script still emits an unsigned development manifest.
- Cross-machine hermetic builds remain unverified.
- Windows execution remains unverified.

## Next requirement

Tighten production release operations:

1. Require an explicit release policy file rather than many ad hoc flags.
2. Bind minimum registry version and required signer roles to policy.
3. Add negative tests for wrong anchor, fork, rollback and missing snapshot.
4. Make the policy summary machine-readable.
5. Keep package promotion closed when policy evaluation is ambiguous.
