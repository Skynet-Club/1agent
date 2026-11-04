# Status v0.94.0

Date: 2026-11-03

## Product state

1A is one provider-neutral runtime with two explicit TUI product forms:

- Standalone 1A: Core conversation, first-principles analysis, evidence, audit, session,
  workspace, context, presets, export and copy.
- Workshop Master-Agent: Core plus bounded mission, planning, assignment, dispatch,
  supervision, human response, control, timeline and independent verification.

## Implemented production capabilities

- deterministic triage and bounded model-generated Work Item DAGs;
- strict plan, risk, permission, workspace and Artifact validation;
- capability-filtered deterministic Agent assignment;
- explicit human approval bound to the plan SHA-256;
- persistent leases, idempotency keys, generation checks and Master journal replay;
- Workshop Session dispatch, polling and bounded human-input progress markers;
- board, watch, attention, response, pause/resume/cancel/takeover/retry/replan and timeline;
- current-filesystem Artifact checks, exact criteria/test evidence and SHA-256 verification;
- verifier identity separated from the assigned execution Agent;
- Master evidence, audit and production report views;
- TUI session lifecycle, workspace/Mention/Git controls, presets and safe exports;
- mutually exclusive `/` command panels that replace prior panel content and return to
  conversation on `Esc`, without appending command views to the transcript;
- keyboard-selectable command submenus, including `/workspace` root selection and
  persistent workspace updates;
- Chinese-first operator panels with grouped labels replacing raw key/value status rows;
- executable command palettes for `/help` and related actions on every panel command;
- character-budget headroom, deterministic local recap/compaction and persisted run
  budgets for cost control.
- journal-derived `/cost` token/cost ledger with provider cost preservation and route
  manifest pricing fallback;
- automatic context compaction at a configurable 50%-95% threshold, defaulting to 80%.
- versioned Pi/Hermes/Claude/Codex capability-probe manifest with hash-only bounded reports
  and pass/fail/blocked classification.

## Automated evidence

```text
library tests: 231 passed
main/TUI tests: 20 passed
CLI E2E tests: 16 passed
other offline integration suites: passed
fmt: passed
clippy -D warnings: passed
rustdoc -D warnings: passed
release build: passed
```

Release binary:

```text
1agent 0.94.0
sha256 864ffaf035af9e205a09a875ddb75319e13709bc009a7a08d3cf6bbfe52f606b
```

The loopback-only OpenAI-compatible adapter test remains ignored in the managed sandbox
because socket binding is denied.

## Real interface evidence

- Full TUI workflow test: mission -> plan -> approve -> assign -> dispatch -> ArtifactReady
  -> independent verify -> Master evidence/report/audit.
- Phase 4 PTY: board/watch/attention/respond/pause/resume/takeover/cancel/timeline.
- Phase 5 PTY: artifacts/verify/evidence/audit/report, final state `verified`, seven audited
  Master events.
- Phase 6 PTY: context headroom/config/compact, workspace init/mention, preset run,
  session rename/fork and session export.
- Command-panel PTY: `/config` was replaced by `/cost` without retaining the old panel,
  and `Esc` restored the underlying conversation surface.
- Workspace-menu PTY: `/workspace` -> `设置工作区` -> second authorized root updated
  `cwd` and persisted session workspace, then refreshed the parent action panel.
- Status PTY: `openai-compatible/deepseek-flash` rendered as a Chinese grouped overview;
  run, session, workspace, model, budget and context fields were readable without raw
  `key=value` rows, and a long panel opened at the top.
- Command-palette PTY: `/help` -> `/status` -> `上下文` executed `/context show` through
  keyboard selection, proving panel actions use the real command and permission path.

## Honest exclusions

- Theme, Vim mode, key remapping and pets are intentionally excluded as non-production
  decoration.
- Character headroom is not provider token accounting.
- External Workshop and authenticated multi-runtime success rates were not re-measured in
  this closing run.
- No production human-productivity baseline is claimed.

## Start here

- `docs/handoff-v0.94.md`
- `docs/audit-v0.94.md`
- `docs/master-agent.md`
- `docs/tui.md`
