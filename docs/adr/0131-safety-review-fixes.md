# ADR-0131: Safety review fixes for approval, MCP, subagents and resume

- Status: accepted
- Date: 2026-11-14
- Supersedes: none
- Related: ADR-0118 (kill switch), ADR-0125 (answer repair), ADR-0130
  (intelligent loop), `docs/audit-v0.97.md`

## Context

An independent code-level review of the 0.97 tree found five P1 classes that
could let a tool run without the intended gate, and the answer-quality packet
found two user-visible failures (conceptual questions routed to Execute, and a
canned "tool budget exhausted" answer). The existing tests did not cover these
paths.

## Decisions

1. **Approval is fail-closed.** `AgentRuntime::execute_tool` accepts only
   `PolicyOutcome::Allow`. `RequireApproval` returns a
   `PermissionDenied` observation with `approval_required`; it never executes
   in the background. A live approval artifact is not wired into this path in
   0.97.
2. **`spawn_agent` is write-capable.** The tool can start a child that writes,
   so read-only mode, the kill switch and write-level Authority checks run
   before the child process starts. The child's own write/network permissions
   remain intersected with the parent's.
3. **MCP trust is local, not server-declared.** Every discovered MCP tool is
   write-gated and requires `--allow-network` at execution. `readOnlyHint` is
   not trusted; `destructiveHint` only raises the risk. Stdio servers are
   started with `env_clear()` plus `PATH`, `HOME`, `TMPDIR`, locale and the
   operator's `server.env`, so provider keys and tokens are not inherited.
4. **Resume fails closed on an uncheckpointed tool call.** If a paired
   `ToolCalled`/`ToolObserved` follows the last checkpoint, `resume` refuses:
   the side effect already happened and replaying the step could repeat it.
5. **Provider redirects are disabled.** A 302 could resend the bearer key to a
   same-host HTTP downgrade; a misconfigured endpoint now fails visibly.
6. **The tool budget gets a final answer.** At the limit, the runtime makes one
   tool-free request using the evidence already in the conversation, then the
   existing schema repair if needed. If both fail it returns the auditable
   `needs_evidence` budget answer.
7. **Conceptual questions are not Execute.** `infer_mode` skips the Execute
   keyword branch when the sentence is a question with no imperative marker,
   and English action verbs use word-boundary matching. "BufWriter flush 的后果"
   is answered; "把文件删掉" stays an action.

## Consequences

- MCP servers that only expose read tools still need write permission and
  network permission in 0.97; an operator-side read-only allowlist is future
  work. This is deliberately stricter than the previous server-hint behavior.
- A crash after a tool call but before its step checkpoint cannot be resumed;
  the user must start a new run or inspect the journal. This trades recovery
  coverage for no duplicate side effects.
- Budget-exhausted runs can now end with `completed` if the final tool-free
  answer validates. The journal records the extra request; cost accounting
  includes it.
- Existing MCP read-only tests and the `tools` command test were updated to the
  new classification.
- Browser SSRF, run-journal hash chaining, cross-builder reproducibility and
  the historical attribution marker remain open; they need larger changes and
  are listed in `docs/audit-v0.97.md`.

## Tests

- `cargo test --offline --lib execution_gate_denies_tools_that_require_approval`
- `cargo test --offline --lib spawn_is_write_capable_so_read_only_gates_cannot_bypass_it`
- `cargo test --offline --lib tool_budget_gets_one_final_tool_free_answer`
- `cargo test --offline --lib resume_rejects_a_tool_call_after_the_last_checkpoint`
- `cargo test --offline --lib mode_inference_is_deterministic_and_requirement_aware`
- `cargo test --offline --lib tools::mcp`
- `cargo test --offline --test tools_e2e`
