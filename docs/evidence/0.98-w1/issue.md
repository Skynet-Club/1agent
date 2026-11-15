# W1 issue: setup silently switches or accepts an unadvertised model

Status: open (real repository maintenance input for 1Agent W1)
Repository: `/home/skynet/1-agent-w1` (detached at `12bf5ea`)
Test command: `cargo test --offline --test setup_cli`
Extra gate: `cargo test --offline --lib onboarding`

## Context

`1agent setup` is supposed to prefer the model the operator already chose.
Today the non-interactive / `--check` path can silently replace the configured
model with the provider's first advertised model, and it never checks whether
the chosen model is actually in `/v1/models`.

This is the same failure class that produced an HTTP 400 in the real
environment: an unadvertised model id was persisted and the next run failed.

## Reproduction

Use a local OpenAI-compatible fake provider that advertises exactly two models:
`fake-model-a` and `fake-model-b`. It must return
`GET /v1/models` and a valid `POST /v1/chat/completions` response.

1. Initialize a runtime home and write a config whose provider model is
   `legacy-model` (for example with
   `setup --non-interactive --provider openai-compatible --base-url
   http://127.0.0.1:PORT/v1 --model legacy-model --skip-probe --skip-smoke`).
2. Run:

   ```bash
   1agent setup --check --root "$ROOT" --probe --json
   ```

Observed:

- the `model` check reports `fake-model-a` (the first advertised model), not
  the configured `legacy-model`;
- the desired state silently switches the model;
- the check reports the model as `ok` even though `legacy-model` is not
  advertised.

Expected:

- explicit `--model` wins;
- otherwise the existing configured model wins;
- only when neither exists should the first advertised model be used;
- if the chosen model is not in the advertised list, report a `model`
  warning; in `--check` mode the report must not be ready (`model` should be
  `missing`, not `ok`);
- do not silently drop an existing configured model.

## Code pointers

- `src/onboarding.rs`, `run_setup`: the model selection block currently
  has the order `options.model -> probed_models.first() -> spec.model ->
  existing_config`. Move the configured model ahead of the first advertised
  model.
- The same file builds the `model` `SetupCheck`; add the advertised-list
  check there.
- `not_ready_or_ready` treats `Missing`/`Fail` as not ready; use `Missing`
  for the `--check` warning.
- `tests/setup_cli.rs` has the existing setup contract tests. A pure unit test
  for the selection order is acceptable if the integration fake-provider test
  is too heavy; add the strongest test that is practical.

## Acceptance

- A regression test fails on the old order and passes on the fix.
- `cargo test --offline --test setup_cli` passes.
- `cargo test --offline --lib onboarding` passes.
- `cargo fmt --check` and
  `cargo clippy --offline --all-targets -- -D warnings` pass.
- No unrelated refactor; smallest diff.
- Do not commit, do not push, do not tag.
- Report the changed files, the test output, and the exact commands used.
