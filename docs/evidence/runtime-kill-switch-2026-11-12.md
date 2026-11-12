# Runtime Kill Switch — Evidence (2026-11-12)

Status: **passed on this machine**
Scope: file-based stop for write-capable tools.

## Command

```bash
cargo test --offline kill_switch --lib
```

## What this proves

- With no switch file, a `write_file` call succeeds and a `read_file` call
  succeeds.
- After creating the switch file, `write_file` returns `PermissionDenied` with
  `error_code = "kill_switch_active"` and does not create the target file.
- `read_file` still succeeds while the switch is active, so the operator can
  inspect evidence.
- The default path is `~/.1a/KILL` for an initialized home and `.1a/KILL` for a
  legacy worktree; `run`, `resume`, the surface runtime and the TUI runtime all
  install it.

## What this does not prove

- OS-level sandboxing, process isolation or credential protection.
- Interruption of a tool that already started before the file appeared.
- That same-UID processes cannot create or remove the file.
- Cross-platform path/permission behavior beyond the shared runtime check.
