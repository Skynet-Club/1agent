# MA-012 OTel child trace (2026-11-20)

Status: **landed; parent and child journals project onto one trace**

## Scope

The journal projection now joins a spawned child's root span to the parent's
`spawn_agent` tool span. The parent and child remain separate journals; the
trace link is reconstructable from `ParentRunLinked` and deterministic ids.

## Landed

- `spawn_span_id(parent_run_id, spawn_call_id)` is a SHA-256-derived 64-bit
  span id (high bit set, so it cannot collide with the low sequential journal
  span ids). The parent projection uses it for the `execute_tool spawn_agent`
  span.
- A child journal with `ParentRunLinked`:
  - uses the parent run's trace id (the parent run id parsed as a UUID);
  - sets the child root `parent_span_id` to the parent's deterministic spawn
    span id;
  - derives its own root span id from the child run id (SHA-256, high bit set)
    so sibling children in one trace do not share `0000000000000001`;
  - sets `gen_ai.conversation.id` to `parent_session_id` when present;
  - adds `gen_ai.subagent.parent_run_id` and
    `gen_ai.subagent.spawn_call_id` attributes.
- Model and tool spans in the child use the child root and the session
  conversation id. A top-level run without a linked session keeps the run id as
  the conversation id.

## Tests

- `child_root_span_joins_the_parent_spawn_span_in_one_trace`: builds a parent
  journal with a `spawn_agent` tool call/observation and a child journal with
  `ParentRunLinked`; asserts the child root and chat span share the parent trace
  id, the child root's parent is the parent spawn span, the child root is not
  the fixed parent id, and both conversation ids are `session-1`.
- Existing `empty_events_emit_no_spans` and `trace_id_is_stable_for_a_run`
  still pass.

Full gates (`gate.txt`, `eval.txt`):

- lib 1,357 passed / 0 failed / 4 ignored;
- bin 67 passed;
- CLI e2e 39 passed with the one pre-existing baseline failure skipped
  (`cli_tui_agents_refresh_uses_configured_profiles`, also fails on unmodified
  HEAD `264fa34`);
- MAST fault injection 7 passed;
- eval `evals/core.json` all scenarios PASS;
- strict `cargo clippy --offline --all-targets -- -D warnings` clean;
- `cargo fmt --check` clean.

## What this does not prove

- This is still a journal projection, not an OTel SDK or live span exporter.
  `1agent otel-export` accepts one journal; exporting a whole tree requires
  projecting each journal separately and joining in the collector.
- A top-level run has no journaled session id, so its conversation id remains
  the run id. Only linked children use the parent session.
- If `parent_run_id` is not a UUID, the child keeps its own trace id.
- Span-id derivation is hash-based; a malicious journal could forge
  `ParentRunLinked`. Journals are local evidence, not a trust boundary.

## Release provenance

Feature commit `5b26da8`; release binary `70033348…` installed as
`~/.local/bin/1agent`; previous MA-011 build `811df54a…` backed up as
`1agent-pre-otel-child-trace-2026-11-20`. Installed smoke: `1agent --version`
and `1agent otel --help` exit 0. Details in `release.txt`; source and evidence
digests in `sha256.txt`.
