# ADR 0118: File-Based Runtime Kill Switch

- Status: accepted
- Date: 2026-11-12
- Related: SA97-18

## Context

The threat model lists the absence of an operator kill switch as a residual
gap. `ops_guard` has an in-memory `trip`, but no runtime construction uses it
and there is no out-of-band file an operator can create while a run or TUI is
live. SA97-18 asks for a kill switch as part of the sandbox/approval work.

## Decision

1. `AgentRuntime::with_kill_switch(path)` installs a file-based switch.
2. Before every tool call, the runtime checks whether the file exists. When it
   does, a tool whose capability requires write returns
   `PermissionDenied` with `error_code = "kill_switch_active"`; read-only tools
   remain available so the operator can inspect the run.
3. The CLI resolves the default path with
   `home_or_legacy("KILL", ".1a/KILL")`, so an initialized `~/.1a` home uses
   `~/.1a/KILL` and old worktrees use `.1a/KILL`. `run`, `resume`, the
   concurrent surface and the TUI runtime all install it.
4. The switch is a local file created and removed by the operator. It does not
   signal, restart or clear locks, and it does not expand authority.
5. The check is per tool call; a long-running tool that already started is not
   interrupted.

## Consequences

- An operator has a simple, out-of-band stop for new write side effects.
- Read tools are deliberately not blocked; stopping reads would remove the
  ability to diagnose the paused run.
- File existence is checked before the tool executes; there is a race between
  the check and an already-authorized write.
- This is not an OS sandbox, credential proxy or process isolation. Those
  remain separate gaps.
- Same-UID processes can create or remove the file; file permissions are the
  operator's responsibility.
