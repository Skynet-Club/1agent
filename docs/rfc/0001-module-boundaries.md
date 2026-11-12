# RFC 0001: Session / Harness / Sandbox Module Boundaries

- Status: **accepted for 0.97 migration planning** (local architecture review;
  no external review)
- Date: 2026-11-12
- Related: `docs/architecture.md`, ADR-0006 (platform branching), ADR-002?
  (permission model), `docs/threat-model.md`

## 1. Problem

Two files own too much host behavior: `src/main.rs` is about 20,300 lines and
`src/tui.rs` about 7,900 lines. They mix argument parsing, provider
construction, command dispatch, session metadata, rendering, input handling
and one-off glue. That makes the domain hard to reuse from the daemon, worker
and future desktop host, and it makes every command change a merge risk.

The 0.97 plan asks for three explicit interfaces — **session**, **harness**,
**sandbox** — and a first split of `main.rs` and `tui.rs`, with no behavior
change.

## 2. Current layers

| Layer | Typical modules | May depend on |
|---|---|---|
| Domain | `domain`, `journal`, `context`, `router`, `provider`, `runtime`, `cua`, `clarify`, `verification`, `memory*`, `skill*` | Other domain modules, `tools` contracts |
| Application | `scheduler`, `production_*`, `worker_runtime`, `master*`, `surface_*`, `inbox*`, `watchdog` | Domain |
| Adapters | `main.rs`, `tui*`, `config`, `tools/*` transports, HTTP | Domain + application |
| Platform | `platform`, `runtime_home`, `tui_terminal`, `tools/terminal` | OS APIs; platform branching is allowed here only |

The dependency rule is already documented: domain and application code must
not depend on the CLI, the TUI or an operating system. `cfg(target_os` is
allowed only in presentation adapters (`scripts/verify.sh` enforces this).

## 3. Three interfaces

These are contracts to extract, not new behavior. Names are provisional.

### 3.1 Session

Owns durable user/host identity and conversation metadata: session id,
preferences, selected workspace, mission/plan summary, journal references and
the append-only session event stream. It does not own the run journal and it
does not render.

Suggested surface:

```text
SessionStore
  open(root, id) -> Session
  create(root, title) -> Session
  list(root) -> [SessionSummary]
  append(event) -> seq
  record_journal(path)
  preferences() -> SessionPreferences
  update_preferences(patch) -> SessionPreferences
```

Existing `tui_session.rs` already implements most of this, but its name and
error types are TUI-shaped. The extraction should move the store into a
host-neutral module and leave the TUI as one consumer.

### 3.2 Harness

Owns one task execution: provider construction, routing, tools, budgets,
cancellation, journal writing and the final `RunResult`. `AgentRuntime` is the
core; the harness is the facade that hides provider/config/journal setup from
hosts.

Suggested surface:

```text
Harness
  new(config) -> Harness
  run(request, cancellation) -> RunResult
  resume(journal, cancellation) -> RunResult
  reframe(task, constraints, cancellation) -> ReframeProposal
```

Hosts must not construct providers or journals directly. CLI, TUI, daemon and
worker call the same harness, so a task behaves the same in every host.

### 3.3 Sandbox

Owns permission and capability decisions for tools: read roots, write,
network, process, browser profile, MCP endpoints, secret access and the
authority/approval gate. It answers `allow | require_approval | deny` and
records the decision; it does not execute the tool.

Suggested surface:

```text
SandboxPolicy
  authorize(ToolCallRequest) -> Decision
  budget(cost, tokens, steps) -> Decision
  redact(text) -> text
  record(decision, receipt)
```

Existing `PermissionSet`, `ExecutionGate`, `approval`, `net_policy` and
`security`-adjacent code are the pieces. The extraction should not add a new
authority source; it makes the existing checks one seam.

## 4. `main.rs` split

Target layout (files may be smaller than the names suggest):

```text
src/main.rs                  // parse + dispatch only
src/cli/mod.rs               // clap command/flag definitions
src/cli/bootstrap.rs         // config, providers, journal dir, home
src/commands/run.rs
src/commands/tui.rs
src/commands/session.rs
src/commands/skills.rs
src/commands/inbox.rs
src/commands/master.rs
src/commands/maintenance.rs
src/commands/eval.rs
```

Rules:

1. One migration slice per command group. Move code, do not rewrite it.
2. Keep clap definitions and handler signatures stable; no flag changes in a
   move-only slice.
3. `main.rs` keeps a thin `match` that calls `commands::*::run`.
4. Each slice must keep `cargo test`, `verify.sh`, the TUI PTY suite and the
   claims table unchanged.
5. If a command is only reachable from one host, move it to that host's module
   instead of the shared command layer.

## 5. `tui.rs` split

Target layout:

```text
src/tui/mod.rs               // re-exports only
src/tui/state.rs             // TuiState and transitions
src/tui/render.rs            // frame layout and panels
src/tui/transcript.rs        // wrapping, visual lines, selection
src/tui/activity.rs          // running line and activity frames
src/tui/input.rs             // input editing/history/keys
src/tui/command_panel.rs     // slash drawer and command panel
```

Order:

1. Extract pure functions first (`wrap_display`, highlighting, layout math)
   with their tests; no state moves.
2. Extract transcript projection and selection.
3. Extract command panel and input editing.
4. Extract state only when the pure pieces are stable.
5. Keep `tui.rs` as a compatibility re-export until the end of 0.97; deleting
   it is a separate decision.

## 6. Non-goals

- No new features, flags, output formats or permission behavior.
- No rewrite of the runtime loop.
- No platform-specific code in domain or application modules.
- No data migration; sessions and journals keep their current formats.
- No big-bang move. Every slice is a revertible commit.

## 7. Verification

Every migration slice runs:

```bash
cargo fmt --all --check
cargo clippy --offline --all-targets -- -D warnings
cargo test --offline
./scripts/verify.sh
./scripts/tui-interaction-check.sh
```

The acceptance for the first batch (SA97-25) is: behavior unchanged, tests
pass, no new `cfg(target_os` outside adapters, and `main.rs`/`tui.rs` each lose
at least one cohesive module.

## 8. Open questions

- Whether `SessionStore` should live in domain or application. The current
  `tui_session` stores UI preferences; the durable session identity may belong
  to application.
- Whether `SandboxPolicy` should wrap `ExecutionGate` or replace it. The safer
  path is a facade that delegates, so authority remains one implementation.
- How the desktop host will consume the harness without linking the CLI crate.
  A library facade plus a thin binary is the working assumption.
