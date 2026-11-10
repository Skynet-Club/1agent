# ADR 0031: Execution and File Writes by Default

- Status: accepted for the local CLI/TUI
- Date: 2026-11-09

## Context

1Agent's V0 rule was read-only by default. In practice the agent could read files
and search a workspace but could not edit a file or run a terminal action, even
when the user explicitly asked for work. The deterministic "needs
authorization" answer became the normal outcome for basic agent tasks, and the
user had to know the hidden `/permissions write on` switch or the `--allow-write`
flag before anything could run.

The user requested that execution be the default: it is the basic capability of
an agent, not an advanced mode. The safety model already provides the necessary
controls: Authority/Policy checks, allowed roots, path and session checks,
journaling, output bounds and a `--read-only` switch.

## Decision

1. `1agent run` and `1agent tui` enable file writes and terminal execution by
   default. `--read-only` disables them for a run.
2. The old `--allow-write` flag remains accepted as a hidden compatibility flag;
   it conflicts with `--read-only` and otherwise has no effect because write
   authority is already on.
3. Surface/API callers and task JSON inputs keep their existing default
   (`allow_write=false`) unless they explicitly opt in. The default change is
   limited to the local CLI/TUI.
4. `write_file` and `edit_file` are added as first-class tools. They are
   write-gated, stay inside the configured roots, reject symlink targets and
   replace files atomically. `terminal_create` and the other terminal tools
   remain high-risk, write-gated adapters over Skynet Workshop.
5. `--allow-network` remains opt-in. Execution does not imply network access.
6. The authorization answer now names `--read-only` and `/permissions write on`
   so a user who disabled execution can re-enable it without guessing.

## Consequences

- A fresh `1agent` can perform the basic loop: read, write, edit and run.
- The safety posture now depends on Authority/Policy, allowed roots and
  journaling rather than a global read-only default. A user who needs a strict
  audit-only run must pass `--read-only`.
- `write_file` is whole-file and `edit_file` is exact replacement; there is no
  structured patch tool yet.
- Terminal execution still requires the Skynet Workshop session daemon and the
  supported Linux x86_64 platform.
- Tests that explicitly construct a read-only `PermissionSet` still exercise the
  denied path; the authorization gate remains covered.
