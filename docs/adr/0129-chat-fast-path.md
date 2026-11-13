# ADR 0129: Chat Fast Path and Tool-Free Direct Answers

- Status: accepted
- Date: 2026-11-13
- Related: ADR-0125, ADR-0128, `docs/cost-governance.md`

## Context

A simple question in the TUI could take 8–21 model calls. The full system
prompt, tool schemas, structured task JSON and multi-step loop were sent even
for greetings and follow-up questions. `infer_mode` also treated any request
with conversation context as `EvidenceResearch`, so a short follow-up almost
never reached `DirectAnswer`.

Measured baseline on 2026-11-13: 21 runs, 102 model calls, 1.25M input tokens.
“启动指令是什么？” used 99,661 tokens across 8 calls; a self-introduction used
4 runs and 11 calls.

## Decision

1. Add `domain::looks_like_chat`, a conservative text classifier. A question
   cue (including Chinese `？`/`吗`/`是什么` and English `?`/`how`/`what`) sends
   a short request to chat unless it also contains an imperative such as `帮我`,
   a retrieval/local/current-fact cue (`查一下`, `文件`, `价格`, `最新`, `项目`,
   `代码`…), or a context reference (`它`, `这个`, `this`, `its`…). A bare
   action, command, attachment, `继续`/`continue` or a request longer than 120
   characters stays on the work path.
2. `infer_mode` no longer treats `known_facts` alone as evidence research.
   Conversation context still reaches the model in every mode; it no longer
   forces the expensive route.
3. `AgentRuntime::run_with_mode` applies the fast path only when the mode is
   `DirectAnswer`, the request has no attachments, no explicit tool query is
   configured, the objective has no context reference, and `looks_like_chat`
   is true. The fast path:
   - uses `provider::chat_system_prompt` (under 1,800 characters), including
     the product's mode and capability facts;
   - sends `tools: []`;
   - limits the run to one step;
   - sends the objective as a plain `User message:` instead of structured
     task JSON;
   - omits experience guidance.
4. If the fast answer returns `needs_evidence`, `run_with_mode` retries once
   through the full agent with tools. Simple chat stays one call; an uncertain
   answer does not get stuck in the tool-free path.
5. `AgentRuntime::run` (benchmark, eval and embedding callers) keeps the full
   loop. The fast path is not used to measure work-task quality.
6. The answer still uses the normal `AgentAnswer` validation and journal
   events. The chat prompt allows only `completed` or `needs_evidence` and the
   minimal shape `schema_version` + `status` + `conclusion`.

## Consequences

- A simple chat turn is one provider call with no tool schema, instead of
  8–21 calls. The expected input is roughly the compact prompt plus bounded
  conversation context (up to 6k characters).
- The classifier is deliberately biased toward work. A misclassified creative
  request may still take the expensive path; it will not lose tools.
- There is no per-call wire `max_tokens` yet. Output length is constrained by
  the minimal-JSON prompt, not a hard provider limit; adding the wire field is
  a follow-up.
- Chat turns still write a journal with usage, so `1agent cost` can attribute
  them. There is no separate `run_profile` event yet; the mode plus one-step,
  no-tool shape identifies the path.
- This is not a lossless optimization. A chat answer cannot read files, search
  the web or verify current facts. The hard work cues and the `needs_evidence`
  escalation bound that loss, but they do not prove answer accuracy. A fixed
  chat/follow-up A/B remains required before claiming quality parity.
- No TUI information-flow change is part of this decision: there is no
  per-turn cost row and no new chat command. The fast path is automatic.
