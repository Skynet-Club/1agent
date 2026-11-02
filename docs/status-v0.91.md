# Status v0.91.0

Date: 2026-11-02
Status: interactive terminal UI implemented

## Interface

`1agent tui` starts a line-oriented full-screen terminal session with a persistent topbar:

```text
1·Agent | status=... | mode=... | risk=... | provider=... | budget=... steps/...s
 You > <last user instruction>
```

Commands:

```text
/help
/clear
/exit
/quit
```

Ctrl+C cancels the active runtime call. Journal, permissions and provider behavior remain owned by
the existing runtime.

## Verification

| Check | Result |
|---|---|
| Unit tests | 129 passed |
| Integration / E2E | passed offline |
| Core evaluation | 6/6 passed |
| topbar preserves instruction | passed |
| running/completed transition | passed |
| control-sequence sanitization | passed |
| CLI TUI workflow | passed |
| manual PTY smoke | passed |
| Clippy `-D warnings` | passed |
| Clean-source package gate | pending |

## Known limitations

- line-oriented input rather than raw-mode editing;
- no mouse, pane navigation or token streaming;
- one active run at a time;
- current topbar is a single terminal session, not persisted across process restarts.

## Next requirement

Add resumable TUI session identity and a command palette for route, evidence, ledger and report views.
