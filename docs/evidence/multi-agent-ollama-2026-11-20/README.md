# 多 Agent + Ollama 冒烟证据（2026-11-20）

Status: **passed on this machine**  
Baseline: `1.0.0-rc.12`, branch `1.1-dev`, HEAD `341c6b2` + Batch 1 working tree  
Provider: local Ollama `v0.40.16`, `qwen3:8b` + `num_ctx 16384` (`qwen3-8b-16k`)  
Endpoint: `http://127.0.0.1:11434/v1`

## Setup

- Ollama binary: `~/.local/share/1agent-ollama/dist/bin/ollama`
- Models: `~/.local/share/1agent-ollama/models`
- Server: nohup process, not systemd; pid in `~/.local/share/1agent-ollama/serve.pid`
- Release archive downloaded through skyget with
  `sha256:87bfafb2acbc89b2e6a1be0a9043de6aa75fb450696f86bc426e65fc448f788d`
- 1Agent env: `AGENT_BASE_URL=http://127.0.0.1:11434/v1`,
  `AGENT_MODEL=qwen3-8b-16k`; loopback needs no API key.

## Run

Parent run id: `0c4da351-2b92-438e-a5d2-0707fb4562d3`

```bash
AGENT_BASE_URL=http://127.0.0.1:11434/v1 AGENT_MODEL=qwen3-8b-16k \
target/debug/first-principles-agent run \
  --provider openai-compatible \
  --base-url http://127.0.0.1:11434/v1 \
  --model qwen3-8b-16k \
  --task "Multi-agent collaboration test. Call the tool spawn_agent exactly twice, one after the other. For each call set allow_write=false, allow_network=false, timeout_seconds=300, and do not pass a workspace. First call task: 'You are Child A. In one sentence, explain why 42 is even. Do not use tools.' Second call task: 'You are Child B. In one sentence, independently verify why 42 is even. Do not use tools.' After both calls, read their conclusions from the tool results and answer with exactly: A=<conclusion>; B=<conclusion>; agree=yes|no." \
  --journal-dir /tmp/1agent-multiagent-ollama/journals \
  --max-steps 10 --max-tool-calls 12 --timeout-seconds 900 --json
```

## Result

- Parent journal: **2** `tool_called spawn_agent` and **2** `tool_observed`
  with status `ok`.
- Child A: run `71f5eb78-c9a8-4140-b8a4-c96b3e00e108`, status `completed`,
  conclusion “42 is even because it divides by 2 with no remainder: 42 = 2 × 21.”
- Child B: run `778e3a86-5c4b-4097-8273-b7bfb6905fd2`, status `completed`,
  conclusion “42 is even because 42 = 2 × 21, i.e., it divides by 2 with no
  remainder.”
- Registry: two final records with `parent_run_id`, `spawn_call_id`,
  `child_run_id`, `delegation_id`; `goal_id`/`work_item_id` are `null` because
  this run had no goal/work-item env.
- Journal audit: parent + both children `ok`, no issues; structural replay
  `ok` for all three.
- Final answer: status `conditional` because `spawn_agent` is write-classified
  and no `acceptance_gate` receipt was produced; conclusion is
  `A=...; B=...; agree=yes`.
- Each child tool result carried a structured `answer` (status, conclusion,
  claims, evidence, conflicts, unknowns) with counts; claims/evidence were
  empty for this simple task.

## What this proves

- 1Agent can run two child agents against a local Ollama model in one parent
  run; parent/child lineage is written to the registry and child journals;
  child journals pass audit/replay; completed children return structured
  answers; the completion gate honestly downgrades a write run without an
  acceptance receipt to `conditional`.

## What this does not prove

- Local 8B model quality, task correctness at scale, or M1 quality.
- Hundreds of agents, cross-process fanout bounds, timeout/cancel/compensation
  paths, acceptance-gate completion, TUI interaction.
- The local model was not evaluated against the 100-question or M1 suites.

## Artifacts

- `run-multi.json` — final answer envelope.
- `registry.jsonl` — filtered final records for the parent run.
- `journals.sha256` — SHA-256 of the parent and both child journals. Raw
  journals stay in `/tmp/1agent-multiagent-ollama/journals/` and
  `~/.cache/1agent-tmp/subagents/`; they contain model text and are not
  committed.
- `audit.txt` — audit/replay output for all three journals.
- `ollama-models.txt` / `ollama-models.json` — installed model list.
