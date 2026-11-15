# 0.98 W1 — Real repository maintenance

Date: 2026-11-15  
Repository: `/home/skynet/1-agent-w1` (clean detached worktree at `12bf5ea`)  
Provider: DeepSeek `deepseek-flash`  
Task: fix setup model selection so an existing configured model is not
silently replaced by the provider's first advertised model, and report an
unadvertised model instead of claiming it is ready.

## Input

- Issue and acceptance criteria: `issue.md`
- Agent task text: `task.txt`
- Agent run command:

  ```bash
  CARGO_TARGET_DIR=/home/skynet/1-agent/target \
  1agent run --task "$(cat docs/evidence/0.98-w1/task.txt)" \
    --journal-dir docs/evidence/0.98-w1/journals \
    --max-steps 30 --max-tool-calls 50 --max-provider-attempts 40 \
    --timeout-seconds 1200 --max-cost-micros 5000000 --json
  ```

## Agent runs

| Run | Budget | Stop reason | Outcome |
|---|---|---|---|
| `a170c231` | 8 provider attempts | `budget_exhausted` | exploration only |
| `1dcc7034` | 20 steps | `budget_exhausted` | exploration only |
| `9753cf1b` | 30 steps / 40 attempts | `budget_exhausted` | implemented the fix and manual E2E; official tests run by the operator |

The final run produced the patch but exhausted its step budget before its own
final report. This is recorded as-is; the operator independently ran the
acceptance tests below.

## Patch

`patch.diff`, `patch.stat`:

```text
src/onboarding.rs | 87 ++++++++++++++++++++++++++++++++++++++++++++-----------
1 file changed, 70 insertions(+), 17 deletions(-)
```

Changes:

- `select_model(explicit, configured, advertised)`: explicit wins, then the
  configured model, then the first advertised model.
- `run_setup` uses that order.
- An unadvertised selected model is `Missing` in `--check` mode (not ready)
  and `Warn` in a normal setup; it is never silently replaced.
- Four unit tests cover the ordering.

## Review follow-up (operator audit)

The first patch conflated `spec.model` (which can be the provider preset
default) with the existing configured model. A fresh DeepSeek setup would have
pinned `deepseek-chat` and reported it unadvertised. The follow-up derives the
configured model from `existing_config` only, adds
`resolve_setup_model(... provider_default ...)` so the preset default loses to
the advertised list, and adds three tests. See `review-followup.md` and
`followup-verification.txt`; the final onboarding test count is 7.

## Independent verification

```text
cargo fmt --check                                  ✅
cargo test --offline --lib onboarding              ✅ 7 passed (4 agent + 3 follow-up)
cargo test --offline --test setup_cli              ✅ 11 passed
cargo clippy --offline --all-targets -- -D warnings ✅
```

Manual fake-provider reproduction (`verification.txt`):

1. `setup --non-interactive ... --model legacy-model --skip-smoke` writes
   `legacy-model` with `model: warn` ("not advertised").
2. `setup --check --probe` reports `model: missing` and `ok: false`, still
   showing `legacy-model`.
3. Re-running setup without `--model` preserves `legacy-model`; it does not
   switch to `fake-model-a`.
4. Explicit `--model fake-model-b` wins and is reported `ok`.
5. Fresh DeepSeek with no `--model`: the preset default `deepseek-chat` loses
   to the advertised list; `fake-model-a` is reported `ok`
   (`followup-verification.txt`).

## Audit and replay

The final journal was audited and replayed with the release binary:

```text
audit.json:  ok=true, event_count=193, unresolved_tool_calls=[], issues=[]
replay.txt:  exit 0
```

## Cost / budget

`metrics.json` over the three runs:

- 3 runs, 67 tool calls, tool success 95.52%, audit complete 100%.
- 1,564,471 input tokens, 14,468 output tokens.
- The local config records cost 0; the provider-side cost is not claimed here.

## W1 gate result

- The patch and its review follow-up were applied to the active `0.98-dev`
  working tree (still uncommitted).
- The main tree passes `./scripts/verify.sh` after the patch (core 8/8,
  claims 251, 227 verified / 16 partial / 8 no evidence).
- W1 acceptance: the listed test command passes; the diff touches only
  `src/onboarding.rs`; model-visible inputs rebuild from the journal; audit
  and replay are clean; no credential/authority violation.

## Residual

- The agent needed three runs and still exhausted the step budget; the patch
  was reviewed and tested by the operator.
- The denied observation was `write_file` outside the allowed write roots
  (`/tmp`); the agent then wrote the fixture under `/home/skynet`. This is the
  permission model working as intended, not a bypass.
- The `sk-super-secret` / `sk-verify...` strings in the journal and replay are
  test fixtures read from `tests/setup_cli.rs`; they are not credentials. The
  real provider key prefix does not appear in the evidence.
