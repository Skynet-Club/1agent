# Status v0.60.0

Date: 2026-11-02
Status: provider attempt budget and failure summaries implemented

## Changes since v0.59.0

- Route trace JSON adds:
  - `provider_failures`;
  - unique normalized `failure_codes`.
- Human route trace displays failure count and codes, not raw error messages.
- `RunBudget` adds `max_provider_attempts` with legacy default `4`.
- CLI run adds `--max-provider-attempts`.
- Effective attempts are clamped to `1..=16`.
- Consumed attempts are stored in checkpoints and restored on resume.
- Reaching the attempt cap before another available route produces a
  `needs_evidence` answer and `BudgetExhausted`.
- Audit explicitly accepts `BudgetExhausted + NeedsEvidence`.

## Verification

| Check | Result |
|---|---|
| Unit tests | 103 passed |
| Integration / E2E | passed offline |
| Core evaluation | 6/6 passed |
| Route-trace failure count/codes | passed |
| Fallback still works under default budget | passed |
| Attempt cap stops second fallback | passed |
| Budget-exhausted answer | passed |
| Budget-exhausted audit | passed |
| Checkpoint attempt count | passed |
| Resume restores attempt count | passed |
| Clippy `-D warnings` | passed |
| Clean-source package gate | passed |

## Known limitations

- Attempts are per run, not per provider or per time window.
- No delay, backoff, jitter or circuit breaker is implemented.
- The hard ceiling is not configurable above 16.
- Provider failures remain provider-local rather than global routing quality signals.

## Verified clean release

```text
source commit: d36df44153712451d3e629b3aa01f4340ee34ccc
git dirty: false

Linux binary:
fbd547162fafe27cd3b6f67c31bd66f1168783e7844e25b0e63ee753419c553c

Windows binary:
ef4698069654c2441b2b3744d8e0830254b6edbdf720acb34dcc3511254ff5c8

Linux tar.gz:
8ebcec5735396c0036760a5de949deb9af57ec11eb92af2759d5d901f5a39040

Windows zip:
f62b1694887628b609a25e606dcf121a0fd4f47178beaf6e4bfa9293263e27c6

Release manifest:
59af3ff64b36fb8681da7f2ce2f8ae97648c09f6af13403e7d04cc4aaea948c6
```

## Next requirement

Expose a bounded backoff policy between retryable provider attempts:

1. add deterministic retry delay policy;
2. respect remaining run time before sleeping;
3. journal delay decisions and budget exhaustion;
4. cap delay duration;
5. test zero-delay and bounded-delay behavior.
