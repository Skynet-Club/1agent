# Status v0.59.0

Date: 2026-11-02
Status: provider failures journaled and audited

## Changes since v0.58.0

- Journal adds `provider_failed`.
- Each failure records:
  - provider ID;
  - model;
  - stable provider error code;
  - retryability;
  - a sanitized message capped at 512 characters.
- Hidden control and bidi characters in failure messages are replaced.
- Replay preserves failures in `provider_failures`.
- Audit rejects a failure without a matching selected route.
- Audit rejects a success response followed by failure in the same attempt.

## Verification

| Check | Result |
|---|---|
| Unit tests | 102 passed |
| Integration / E2E | passed offline |
| Core evaluation | 6/6 passed |
| Retryable failure event | passed |
| Non-retryable failure event | passed |
| Replay failure order | passed |
| Failure message bound | passed |
| Hidden-control replacement | passed |
| Unselected-route failure audit | rejected |
| Success-then-failure audit | rejected |
| Clippy `-D warnings` | pending |
| Clean-source package gate | pending |

## Known limitations

- Failure messages are sanitized and bounded but can still contain sensitive text supplied by an
  in-process provider.
- The journal does not yet record retry backoff or global retry budget.
- Provider failures are not summarized in the human-readable route trace.
- HTTP identity mismatch remains a separate invalid-output path.

## Next requirement

Expose provider failures in route traces and enforce a per-run attempt budget:

1. show failure count and stable codes without raw messages;
2. add a bounded maximum provider attempts per run;
3. journal budget exhaustion before terminal output;
4. test multi-provider fallback and exhaustion;
5. preserve replay compatibility for older journals.
