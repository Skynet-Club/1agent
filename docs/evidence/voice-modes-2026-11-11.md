# Partner Voice Modes — Evidence (2026-11-11)

Status: **passed on this machine**  
Scope: `voice::instruction`, `voice::with_voice`,
`provider::agent_system_prompt_for`, `AgentRuntime::with_voice_mode` and
`1agent run --voice-mode`. This is a contract fixture, not a blind quality
evaluation.

## Commands

```bash
cargo test voice --lib
cargo test --test voice_mode_cli
cargo clippy --all-targets -- -D warnings
```

## Expected results

```text
running 4 tests
test voice::tests::each_mode_has_a_distinct_instruction ... ok
test voice::tests::advisor_keeps_the_tradeoff_and_flip_condition ... ok
test voice::tests::companion_is_warm_but_not_sycophantic_or_role_playing ... ok
test voice::tests::operator_leads_with_result_and_next_action ... ok

test result: ok. 4 passed; 0 failed

running 2 tests (tests/voice_mode_cli.rs)
test voice_mode_reaches_the_journaled_system_prompt ... ok
test invalid_voice_mode_fails_closed ... ok

test result: ok. 2 passed; 0 failed
```

## What this proves

- Each mode has a distinct instruction; all keep the base system prompt.
- `advisor` keeps the tradeoff and the "what would change it" line.
- `companion` is warm but contains no flattery or role-play markers.
- `operator` leads with the result and next action.
- `--voice-mode advisor|companion` reaches the journaled system prompt.
- An invalid mode fails closed before the run starts.

## What this does not prove

- That real users prefer one mode over another; no blind evaluation was run.
- That the model follows the voice block on every provider; this is a prompt
  contract, not a model-quality result.
- That companion mode is emotionally appropriate in every context; it is
  deliberately bounded to one sentence of context plus the answer.
- Cross-platform behavior; the test ran on Linux only.

## Files

- `src/voice.rs`
- `src/provider.rs` (`agent_system_prompt_for`)
- `src/runtime.rs` (`voice_mode`, `with_voice_mode`)
- `src/main.rs` (`run --voice-mode`)
- `tests/voice_mode_cli.rs`
- `docs/adr/0040-partner-voice-modes.md`
- `docs/architecture.md`, `docs/features.md`, `docs/features.en.md`,
  `docs/claims.md`, `CHANGELOG.md`
