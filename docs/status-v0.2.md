# Status v0.2.0

Date: 2026-11-01
Status: V0 vertical slices for evidence, audit and L2 experience evolution

## Added since v0.1.0

- Experience candidate schema for memory, skill and route-policy lessons.
- Source-run and evidence-event binding: a candidate cannot be proposed from an unaudited run.
- Candidate lifecycle: `draft → reviewed → approved/rejected → active → degraded/rolled_back`.
- Approval is mandatory before activation.
- Experience history is append-only JSONL and projected deterministically.
- Invisible Unicode controls and empty material are rejected.
- CLI lifecycle commands under `experience`.
- Windows GNU cross-target compilation check passed.

## Verification

| Check | Result |
|---|---|
| Unit tests | 17 passed |
| CLI run/audit/replay | passed |
| Experience CLI lifecycle | passed |
| OpenAI-compatible loopback fixture | passed outside managed sandbox |
| Linux release build | passed |
| `x86_64-pc-windows-gnu` all-target check | passed |
| Clippy `-D warnings` | passed |

Platform interpretation:

- Linux: executable behavior verified for core CLI, file read, journal, audit and experience.
- Windows: compilation compatibility verified; runtime, filesystem, packaging and cancellation are untested.
- macOS: untested in this environment.

## Requirement comparison

| Requirement | Status |
|---|---|
| Experience candidates | implemented |
| Source/evidence binding | implemented |
| Review and approval gate | implemented |
| Versioned activation/rollback | partial: immutable append history and lifecycle exist; patch/version promotion is pending |
| Held-out A/B evaluation | pending |
| Candidate retrieval/injection into agent runs | pending |
| Automatic candidate generation | pending; deliberately not enabled |

## Known risks

- Candidate history is single-writer JSONL and has no explicit file lock.
- `sensitive` is recorded but no separate redaction pipeline exists yet.
- Candidate usefulness is not empirically tested; approval alone is not evidence of positive transfer.
- Route-policy candidates are records only and do not alter runtime routing.
- No provider fallback chain exists yet.

## Next requirement

Implement deterministic provider fallback:

1. Rank all eligible provider candidates instead of returning only one.
2. Try the next candidate on retryable network, timeout, rate-limit and provider-unavailable failures.
3. Never fall back on authentication, cancellation or invalid structured output.
4. Journal every route attempt and fallback reason.
5. Keep task state, messages and tool observations unchanged across fallback.
6. Add failure-injection tests using local fake providers.
