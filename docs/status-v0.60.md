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
| Clippy `-D warnings` | pending |
| Clean-source package gate | pending |

## Known limitations

- Attempts are per run, not per provider or per time window.
- No delay, backoff, jitter or circuit breaker is implemented.
- The hard ceiling is not configurable above 16.
- Provider failures remain provider-local rather than global routing quality signals.

## Next requirement

Expose a bounded backoff policy between retryable provider attempts:

1. add deterministic retry delay policy;
2. respect remaining run time before sleeping;
3. journal delay decisions and budget exhaustion;
4. cap delay duration;
5. test zero-delay and bounded-delay behavior.
