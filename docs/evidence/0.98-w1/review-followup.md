# W1 review follow-up

Date: 2026-11-15  
Trigger: operator review of the agent patch (W1 audit step)

## Finding

The first patch passed `spec.model` as the "configured" model to
`select_model`. `spec.model` is not only the existing config model: for a
fresh provider it is the preset default (for example `deepseek-chat`). That
made a fresh DeepSeek setup with no `--model` prefer `deepseek-chat` over the
provider's advertised `deepseek-flash`/`deepseek-v4-pro`, then report
`deepseek-chat` as unadvertised. This would break the default onboarding path.

## Fix

- Derive `configured_model` only from the existing config's default provider,
  not from `spec.model`.
- Add `resolve_setup_model(explicit, configured, provider_default, advertised)`:
  explicit → configured → first advertised → provider default.
- Add three unit tests, including the fresh-provider default case and the
  "provider default only when nothing is advertised" case.

## Verification

`followup-verification.txt`:

```text
fresh DeepSeek, no --model: provider_probe ok (2 models), model ok (fake-model-a)
existing legacy-model: model missing ("not advertised")
rerun without --model: model warn, persisted legacy-model
```

```text
cargo test --offline --lib onboarding   ✅ 7 passed
cargo test --offline --test setup_cli   ✅ 11 passed
cargo fmt --check                       ✅
cargo clippy --offline --all-targets -- -D warnings ✅
```

The follow-up is applied to the active `0.98-dev` working tree; the full
`./scripts/verify.sh` is re-run after it.
