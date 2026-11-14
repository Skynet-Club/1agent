# Terminal execution evidence — 2026-11-09

Provider: `openai-compatible` / `deepseek-flash`  
Host: Linux x86_64, single machine  
Command shape: `1agent run --allow-write --task "<objective>" --json`

## Echo run

Objective: create a terminal session, run `printf 1A_LIVE_OK`, read the output,
destroy the session.

Observed tool sequence:

```text
terminal_create -> terminal_exec -> terminal_output -> terminal_destroy
```

Result: `stop_reason=completed`; the command exited 0 and the output contained
`1A_LIVE_OK`. The model then re-read the destroyed session and got
`session_not_found`, which confirms the destroy step.

## Copilot / GitHub run

Objective: create a terminal session, run `copilot --version`, run
`gh auth status`, read the output, destroy the session, and never print a token.

Observed tool sequence:

```text
terminal_create -> terminal_exec -> terminal_exec -> terminal_destroy
```

Result: `stop_reason=completed`; Copilot CLI reported `1.0.107`; `gh auth status`
reported an active `github.com` account. `gh` masked the token itself
(`gho_****`). No interactive login was attempted because this machine was
already authenticated.

## TUI permission flow

The TUI was started without `--allow-write` inside a tmux PTY. Sending
`/permissions write on` printed `下一次运行允许写入和执行。`; the next task ran
in Execute mode and used:

```text
terminal_create -> terminal_exec -> terminal_destroy
```

to run `echo 1A_TUI_WRITE_OK`. Both TUI run journals record `allow_write=true`.

## TUI default execution

After execution became the default, a fresh TUI was started with no permission
flags. A plain Execute task ran without any `/permissions` command:

```text
terminal_create -> terminal_exec -> terminal_destroy
```

`echo 1A_DEFAULT_ON` returned `1A_DEFAULT_ON` with exit 0 and the session was
destroyed. Its run journal records `allow_write=true` by default.

A second TUI task checked whether `1agent` was globally available. It found
`/home/skynet/.local/bin/1agent`, the existing `~/.bashrc` line 123 and
`~/.profile` line 26 PATH entries, made no changes, and both files still matched
their pre-test SHA-256.

## Artifacts

| File | SHA-256 |
|---|---|
| `echo-run.jsonl` | `807b81347bf2755eb9f2b7e7cf5177e8938ecd9845970a722983df1cd912b731` |
| `copilot-run.jsonl` | `580baf78b8895c2d7b833fcf1756c70af1ade3e4ec91a9b017efef985f72691c` |
| `tui-write-enable-run.jsonl` | `c2ffa67816e2fecfb3d8f52fd31443bd5b3419e272959d01d9921e6645b2742f` |
| `tui-global-path-run.jsonl` | `93e11ae5a8d24b7b97068c30144da8bef97675dbc93f62c8ae4b9c61826f8dea` |
| `tui-default-execution-run.jsonl` | `39e81161df13fc896d4604822623a95b1980a28c441a65c00a37f9b2f65a572b` |

The journals are the raw run records. The only credential-shaped strings are
`gho_` followed by asterisks; no alphanumeric token is present. The TUI
journals contain no credential-shaped strings.

## Caveats

- One machine and one provider; no cross-machine or cross-provider run.
- `gh` was already logged in, so this does not prove an interactive device-code
  login. It proves that 1A can start `copilot`, inspect `gh auth status`, and
  read the result inside a real PTY.
- The live test is `tests/terminal_tools_live.rs` and is ignored by default; it
  requires `skynet-term-sessiond` and `skynet-workshop-session`.
