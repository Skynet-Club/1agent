# Fast delivery vs Pi — tuned prompt

Status: **last complete fast/Pi N=3 run before the read-only acceptance
follow-up; the final read-claim run was paused.**

- Binary: `~/.local/bin/1agent` sha256 `cdccc7c8…` for this run.
- Suite: `evals/real-tasks-v1.json`, 8 tasks × N=3.
- Model/endpoint: `deepseek-flash` via `http://127.0.0.1:8787/v1`.
- Fast arm: `1agent run --fast` (tiny prompt, four tools, plain-text final,
  no in-run acceptance, post-run queue acceptance).
- Pi 1.5.2 default tools in the Docker sandbox.

## Result

| Arm | Neutral completion | Acceptance/status met | Model calls (median) | Tool calls | Wall ms | TTFT ms | Fixed tokens | Prompt tokens | Output tokens |
|---|---|---|---|---|---|---|---|---|---|
| fast | **24/24** | 18/24 | **3.0** | **2.0** | **4,223** | 1,010.5 | 813 | **4,587.5** | 476 |
| pi | 24/24 | — | 4.0 | 3.0 | 6,010 | 809 | 596 | 7,501.5 | 508 |

Fast is **1.78s faster** than Pi on this suite with the same neutral
completion, one fewer model call and one fewer tool call. Fixed prompt/schema
is 217 tokens higher; total prompt tokens are 39% lower.

The 6 status misses are `broken-json` and `directory-summary` read-only runs:
the answer and citations are correct, but the post-run acceptor had no write
artifact from which to derive evidence. Code to derive source claims from
`read_file` observations was added after this run; it was not re-benchmarked
because work paused.

## Tuning that produced this run

The first fast run was 20/24 with 6.85s median because the model wandered
through `terminal_create` and directory exploration until the 8-call budget.
The tuned prompt adds:

- work only inside the task workspace; never inspect the repo/home/parents;
- read with `read_file`, not terminal listing;
- `write_file` returns the sha256; do not re-read or re-verify a successful write;
- one terminal session, only when a command is required;
- read-only tasks answer after reading the requested files;
- the conclusion names the files used and includes every requested value/hash.

## Evidence

`summary.md`, `results.json`, and the raw run tree (ignored by Git).
