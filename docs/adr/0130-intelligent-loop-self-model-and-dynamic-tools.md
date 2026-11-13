# ADR 0130: Intelligent Loop — Live Self Model, Dynamic Tools and Evidence Re-planning

- Status: accepted
- Date: 2026-11-13
- Related: AG-23a–f, 0.97 release gate, M1/M5

## Context

0.97 adds a release gate for the "intelligent loop" (AG-23). The runtime
already had a `ToolRegistry`, `tool_search`, `ClarifyTool`, `verification`
and `ExperienceStore`, but the model still depended on:

- a static `CAPABILITY_CATALOG` for self-introduction;
- a tool list frozen before the first model request;
- schema-only answer repair, with no evidence check or re-plan;
- keyword routing (`looks_like_chat`/`infer_mode`) as the only way to decide
  whether a task needed tools.

The gate rejects a system that can be passed by reciting capabilities or by
keyword matching. The model must read live state, request missing tools and
correct an unsupported answer before it is presented as complete.

## Decision

1. **Live self model.** `describe_self` is a runtime-provided tool. It renders
   the permission-filtered `ToolRegistry`, the selected provider/model and the
   run budgets from the same capability snapshot the journal records. The
   static catalog is deleted; capability questions skip the tool-free chat
   path.
2. **Dynamic tools.** `request_tools` is always offered on the full-agent path.
   The model names the tools it needs; the runtime checks write/network
   permission, appends the schemas to the next request and journals
   `ToolsRequested { requested, granted, denied, reason }`. Invalid, unknown or
   forbidden requests fail closed with a tool observation.
3. **Task need assessment.** `AgentAnswer.task_need` records
   `freshness/locality/action/risk/ambiguity/budget`. A completed answer whose
   own assessment says it needed current/local facts or an action, but carries
   no evidence or action receipt, is not accepted as complete.
4. **Bounded evidence re-plan.** The runtime sends the draft back with an
   `EVIDENCE CHECK` instruction, at most twice. If the evidence still does not
   appear, the answer is downgraded to `needs_evidence`. The chat fast path
   escalates to the full agent when its own assessment contradicts the
   tool-free draft. Journal events `AnswerVerificationFailed` and
   `AnswerVerificationExhausted` record every round.
5. **Strategy memory.** A successful tool-using run with `--experience-dir`
   writes an inert `ExperienceKind::Strategy` draft into the existing
   `ExperienceStore`: mode, objective hash, task need, tool sequence,
   verification rounds and outcome. It never stores the user's text or the
   answer. Drafts go through the existing review/verify/activate/rollback
   lifecycle; identical drafts are deduplicated.
6. **Fixed 100-question evaluation.** `evals/agent-loop-v1.json` contains 20
   self-introduction, 20 current/local, 20 follow-up, 20 simple-chat and 20
   high-risk prompts. `EvalExpectation` gained tool, provider-call and accepted
   status checks; `--artifacts-dir` keeps per-case journals and a `cases.jsonl`
   index so the report can be replayed.

## Consequences

- The model can change its own tool set mid-loop, but only within the run's
  permissions. `request_tools` does not bypass Authority/Policy or the kill
  switch.
- Capability snapshots now include the two virtual tools, so journals and
  resume checks describe exactly what the model saw.
- A resumed checkpoint created before this ADR will fail the capability hash
  check. That is the expected 0.97 rollback boundary.
- Evidence re-planning costs at most two extra model calls per unsupported
  completed answer; simple chat stays at one call and zero tools.
- Strategy drafts are inert until reviewed. Recording is best-effort and never
  fails a run.
- The 100-question suite is a model-quality suite; it requires a real provider
  with `--config` and is not a deterministic contract fixture.

## Verification

- Unit/runtime tests cover live self-model rendering, permission-filtered tool
  requests, network denial, bounded evidence re-planning, fast-path escalation,
  strategy drafting and the fixed 100-case inventory.
- `cargo fmt --check`, `cargo clippy --all-targets -- -D warnings` and the full
  test suite must pass before release.
- The live report and journals are stored under
  `docs/evidence/agent-loop-v1-2026-11-13.json` and
  `docs/evidence/agent-loop-v1-2026-11-13/`.
