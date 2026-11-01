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
| Clippy `-D warnings` | passed |
| Clean-source package gate | passed |

## Known limitations

- Failure messages are sanitized and bounded but can still contain sensitive text supplied by an
  in-process provider.
- The journal does not yet record retry backoff or global retry budget.
- Provider failures are not summarized in the human-readable route trace.
- HTTP identity mismatch remains a separate invalid-output path.

## Verified clean release

```text
source commit: ddf15422dc415d1e90dc7eace3c5f54e80c1092d
git dirty: false

Linux binary:
4851eaf203052fd0567b195286df2da62d8bb3c52f38ebb7a0e317860e8b2dce

Windows binary:
c85c14c93f4f56259201265b2fea7f876c7fc0b140480cd0fda2168bc3ec71ae

Linux tar.gz:
1a191b61e92c4d3fa1c8b8bb6f47d19974ea18c516bd7a82d37e5d720f7f0aa5

Windows zip:
91b6abd4f8e10bb6892fea34bd4c19e3184be0a37156eb94dbb631bdc5b81c91

Release manifest:
1af8154f1f8ab984f12b6452d9b7f8b506ef246114a254976838bd8e3409c912
```

## Next requirement

Expose provider failures in route traces and enforce a per-run attempt budget:

1. show failure count and stable codes without raw messages;
2. add a bounded maximum provider attempts per run;
3. journal budget exhaustion before terminal output;
4. test multi-provider fallback and exhaustion;
5. preserve replay compatibility for older journals.
