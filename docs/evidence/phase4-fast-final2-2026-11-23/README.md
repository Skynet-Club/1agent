# Fast delivery vs Pi — final N=3

Status: **complete; fast mode reaches Pi-level speed with 24/24 neutral and
24/24 post-run acceptance.**

- Binary: `~/.local/bin/1agent` sha256 `3e6f3dcd…`.
- Suite: `evals/real-tasks-v1.json`, 8 tasks × N=3.
- Model/endpoint: `deepseek-flash` via `http://127.0.0.1:8787/v1`.
- Fast arm: `1agent run --fast` (four tools, compact prompt, plain-text final,
  no in-run acceptance, post-run queue acceptance).
- Pi 1.5.2 default tools in the Docker sandbox.

## Result

| Arm | Neutral completion | Acceptance/status met | Model calls (median) | Tool calls | Wall ms | TTFT ms | Fixed tokens | Prompt tokens | Output tokens |
|---|---|---|---|---|---|---|---|---|---|
| fast | **24/24** | **24/24** | **3.0** | **2.0** | **3,872** | 790 | 813 | **4,542.5** | 454 |
| pi | 24/24 | — | 4.0 | 3.0 | 6,189.5 | 653 | 596.5 | 7,509.5 | 586 |

Fast is **2.32s faster (37.5%)** than Pi with equal neutral completion and full
post-run acceptance, one fewer model call and one fewer tool call. Fixed
prompt/schema is 217 tokens higher; total prompt tokens are 39.5% lower.

## What made the difference

- Fast prompt: work only in the task workspace; direct `read_file`; write once;
  no second verification pass; one terminal session only when a command is
  required; conclusion includes file names and requested values/hashes.
- Post-run receipt: verifies the delivery workspace snapshot, write/edit
  artifacts, verified artifacts and `read_file` sources. `read_file` metadata
  uses `relative_path`, which the acceptor now reads.
- `1agent run --fast` implies `--defer-accept`; the queue lives under
  `~/.1a/acceptance/` and is drained with `1agent accept --queue`.

## Evidence

`summary.md`, `results.json`, and the raw run tree (ignored by Git).
