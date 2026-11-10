# Subagent evidence — 2026-11-09

Provider: `openai-compatible` / `deepseek-flash`  
Host: Linux x86_64  
Command shape: `1agent run --task "<objective>" --json`

## Two isolated child runs

The parent agent was asked to use `spawn_agent` for `1+1`. It spawned two
independent children (one with justification, one asking for only the number):

```text
spawn_agent -> spawn_agent
```

Both returned `completed`; conclusions were `1+1=2` and `2`. The parent's
attempt to `read_file` the child's isolated workspace was denied because it was
outside the parent's read roots, confirming the workspace boundary.

The parent run finished with `stop_reason=completed`.

## Artifact

| File | SHA-256 |
|---|---|
| `two-children-run.jsonl` | `8920743a986bf67b6371bdedb36f705438cf4998dcc6b3753ea567f1c3f8e46e` |

## Caveats

- Children inherit `AGENT_*` provider environment; without it they fall back to
  the deterministic mock provider.
- Depth is capped at 2. Child journals live in isolated cache workspaces; the
  parent sees only the returned conclusion and journal path.
- Cancellation kills the child process tree through `kill_on_drop`, but a child
  already running tools may leave its own child processes to their normal
  timeouts.
