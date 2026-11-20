# MA-007 subagent tool allowlist (2026-11-20)

Status: **landed; tool-allowlist slice; deterministic tests pass**

## Scope

MA-007's enforceable target is the child's tool set. `required_capabilities`
(Work Item assignment metadata) has no runtime capability-to-tool mapping, so
this slice **does not claim** to enforce capability names. That is recorded as a
remaining gap, not hidden.

## Landed

- `spawn_agent` accepts an optional exact `allowed_tools` array (max 32 names,
  64 chars each, ASCII alphanumeric plus `_`, `-`, `.`).
- The parent runtime's allowlist is propagated through
  `RunContext.allowed_tools`:
  - parent allowlist + child list -> intersection;
  - parent allowlist + omitted child list -> inherit the parent list;
  - no parent allowlist + child list -> apply the child list;
  - empty array -> deny every tool;
  - an unknown name fails before spawn.
- The child receives repeatable `--allowed-tools` or `--deny-all-tools`.
- `ToolRegistry::restrict_to`:
  - rejects unknown names (virtual `describe_self`/`request_tools` are accepted
    but do not add registry tools);
  - filters `specs`, `specs_for`, `capabilities`, `capabilities_for` and the
    capability snapshot;
  - returns `tool_not_allowed` on an execute call for a hidden tool, even if a
    stale/hallucinated call reaches the executor;
  - `requires_write`, `risk`, `reversible` and `parallel_safe` treat hidden
    tools conservatively.
- The parent's restriction cannot be widened by a child request: the
  intersection is the only path. A child cannot use `request_tools` to add a
  hidden tool because its capability list is already filtered.
- Virtual meta-tools (`describe_self`, `request_tools`) stay visible. They can
  describe the filtered registry and request allowed tools, but cannot reveal
  or activate a tool outside the allowlist.

## Tests

- `tool_allowlist_hides_and_rejects_unlisted_tools`: hidden spec/capability,
  execute returns `tool_not_allowed`, unknown name rejected, `allow_none`
  hides everything, virtual names accepted in an allowlist.
- `child_tool_allowlist_is_intersected_with_the_parent`: parent
  `{read_file, echo}` + child `{read_file, write_file}` -> child argv only
  `read_file`.
- `omitted_child_allowlist_inherits_the_parent_allowlist`: parent
  `{read_file}` -> child gets `read_file`.
- `empty_child_allowlist_denies_all_tools`: child argv has
  `--deny-all-tools`, not `--allowed-tools`.
- `invalid_child_allowlist_is_rejected_before_spawn`: names with spaces, empty
  strings and non-strings are rejected.
- `runtime_advertises_only_tools_allowed_by_the_registry_allowlist`: the real
  runtime request advertises `echo` and hides `read_file`.
- `cli_rejects_unknown_allowed_tool_before_running`: the CLI fails before any
  provider call with `unknown tool(s) in --allowed-tools`.

Full gates: lib 1,338 passed / 0 failed / 4 ignored; bin 67 passed; MAST 7
passed; CLI e2e 40 passed with an isolated `ONEAGENT_HOME`; strict
`cargo clippy --all-targets -- -D warnings` clean.

## What this does not prove

- No live-model spawn test. The fake child records its argv; the registry and
  runtime tests prove the filter and denial. This is a mechanism proof, not a
  model-quality benchmark.
- `required_capabilities` is not enforced at spawn. Enforcing it needs a
  defined capability-to-tool (or capability-to-runtime) mapping.
- The allowlist is tool-level, not an OS sandbox. If a tool is allowed, its own
  permission checks still govern; if `terminal_exec` is allowed with write, the
  child can still run commands within its permissions.
- MCP tools are not inherited by the child process. A parent allowlist that
  names an MCP-only tool makes the child fail closed with an unknown tool.
- Virtual meta-tools remain visible even when the real-tool allowlist is empty.

## Evidence

- `allowlist-tests.txt` — 8 focused tests.
- `cli-test.txt` — unknown-name CLI rejection.
- `metrics.json` — counts and policy.
