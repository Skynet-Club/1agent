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
| Release-profile tests | 129 unit + all E2E passed |
| Clean-source package gate | passed |
| Packaged Linux binary smoke | passed |
| Historical manifests v0.23–v0.91 | all verified |

## Release artifacts

Source commit: `0b3a6081370ac85571224e9d9a99d2a0020c7f42`

Clean-source artifacts were rebuilt after the feature commit. SHA-256:

| Artifact | SHA-256 |
|---|---|
| Linux `tar.gz` | `df0517769e151540f44e7fdbb402893aefc47e353368f8ecc06ec65c882c7014` |
| Windows `zip` | `dada3c3a00ea82061cfda3c4429ea2d522694caf7835500f8c0b04052fdf67ab` |
| Release manifest | `8cea42dd59ddbf9c05df23bb7057e46a14d8cc144b24b53619faa2dcf84a19d8` |
| Linux binary | `9acbad939dc9a127d3e1cfde621c18674f35202af238903b6113794fb9eff134` |
| Windows binary | `3e83dbf0c463dba0b82c770c3079493364822645dcbce069c8adcf415f81ad7d` |

## Packaged binary smoke

`dist/1agent-0.91.0/linux-x86_64/1agent` reported version `0.91.0`, executed the core suite
at 6/6, and completed a scripted TUI task with the expected topbar and journal path.

## Known limitations

- line-oriented input rather than raw-mode editing;
- no mouse, pane navigation or token streaming;
- one active run at a time;
- current topbar is a single terminal session, not persisted across process restarts.

## Next requirement

Add resumable TUI session identity and a command palette for route, evidence, ledger and report views.
