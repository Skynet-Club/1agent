# ADR 0040: Partner Voice Modes — Operator, Advisor, Companion

- Status: accepted
- Date: 2026-11-11

## Context

The existing answer contract is deliberately terse and result-first. That is
right for execution, but a partner also needs to advise and, sometimes, to
acknowledge context. One fixed voice cannot do all three without either sounding
cold or padding every answer.

The risk is the opposite failure: a "companion" that flatters, role-plays or
pretends to feel. The product explicitly rejects emotional manipulation and
role-play.

## Decision

1. Add `src/voice.rs` with three instructions:
   - `operator` (default): result and next action first; no greetings, praise
     or role-play.
   - `advisor`: recommendation first, then the tradeoff that matters and what
     would change it; no confidence hiding, no flattery.
   - `companion`: direct and warm; one sentence of context, then the result or
     next action; no flattery, no role-play, no emotional padding, no pretending
     to feel.
2. `with_voice(base, mode)` appends a `VOICE MODE` block to the existing system
   prompt. The base prompt and all evidence/authority rules are unchanged.
3. `provider::agent_system_prompt_for(mode)` is the single builder.
   `AgentRuntime` carries a `VoiceMode`, defaulting to `operator`, and
   `with_voice_mode` selects another mode.
4. `1agent run --voice-mode operator|advisor|companion` selects the mode. An
   invalid value fails closed before the run starts.
5. Voice changes how the answer speaks, not what counts as evidence. A
   companion answer still cannot invent a tool result, source or action.
6. Voice v1 adds a task form derived from the cognitive mode:
   - `decision` for `DecisionAnalysis` and `Escalate`: recommendation first,
     decisive tradeoff and flip condition;
   - `research` for `EvidenceResearch` and `Diagnosis`: observed evidence,
     inference and unknown separated, with a falsification condition;
   - `execution` for `DirectAnswer` and `Execute`: what changed, verification
     and failures first, with a receipt for every claimed action.
   `agent_system_prompt_for_form` appends the form after the voice block;
   `with_voice` keeps the execution form as its default.
7. The TUI `/why` command renders the latest structured answer's conclusion,
   claims with sources, evidence, assumptions, unknowns, flip conditions and
   action receipts. It reads the journal replay; it does not call a provider
   and does not create a new answer.

## Consequences

- A user can choose execution, advice or companionship without a separate
  product persona.
- The mode is journaled in the model request, so a run can be replayed with the
  same voice.
- The companion mode is deliberately bounded; warmth is one sentence, not a
  personality performance.
- A blind evaluation of the three modes and three forms is still needed; this
  ADR establishes the contract, not a quality claim.
- The form is derived from the cognitive mode, so a user cannot pick a
  mismatched form; changing the mode changes the form.
- `/why` shows the structured answer, not hidden chain-of-thought. It reads
  only what was already journaled.

## Evidence

```bash
cargo test voice --lib
cargo test --test voice_mode_cli
cargo test --bin first-principles-agent why_tests
```

The unit tests cover distinct instructions, the advisor tradeoff, companion
warmth without sycophancy/role-play, the operator result-first rule, the
cognitive-mode-to-form mapping and form composition. The CLI test asserts the
selected mode and execution form reach the journaled system prompt and that an
invalid mode fails closed. The binary tests cover `/why` sections and omission
of empty sections.
