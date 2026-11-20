# Planner reading + skill discovery live run (2026-11-20)

Status: **passed on this machine**  
Model: DeepSeek `deepseek-flash`  
Workspace: `/tmp/skynet-workshop-parallel-dev` (clone of skynet-workshop at `e71ac0e`)  
Plan: `plan.json` (2 Work Items)

## What was tested

`1agent master plan` was given one task:

> Read `docs/agent-registry.md`; use `skill_search` to find the ponytail skill and read
> its body; then propose a Work Item to add `--count` to `skynet-term-agents list/find`,
> with ponytail constraints in the objective. Do not execute.

The planner had only read-only tools: `read_file`, `search_files`, `read_document`,
`skill_search`. It had no write, terminal, network or skill-execution tool.

## Result

- The planner called `read_file` on `docs/agent-registry.md` and received the actual
  registry documentation.
- It called `skill_search` with `query=ponytail`; the tool found **6** matching skills
  from `~/.claude/plugins/marketplaces/ponytail/...` and returned their bodies.
- It used `search_files` and `read_file` to inspect `src/bin/skynet-term-agents.rs`,
  `tests/agent_registry_cli.rs`, `src/agents.rs` and `src/mcp.rs`.
- It called `read_document` on `src/mcp.rs` (text path) successfully.
- It returned a valid 2-Work-Item PlanProposal. Both objectives explicitly reference
  `ponytail (full 级)` constraints, and the second Work Item is a minimal docs sync.
- Every tool call and observation is in the planning journal (`journal.sha256`); the
  planner prompt/response pairs are journaled too.

See `tool-trace.txt` for the ordered tool calls and observations.

## What this proves

- The `master plan` generator is no longer a single no-tool model turn: it can read
  requirements before planning and every read is journaled.
- Skill discovery is generic: the planner found a user-installed skill from a
  `SKILL.md` root, read its body, and carried its constraints into the plan.
- No skill was hardcoded into the product; `lark-cli` is not special-cased.
- The planner stayed read-only: only `read_file`, `search_files`, `read_document` and
  `skill_search` were advertised and used.

## What this does not prove

- The plan was not executed; Work Item dispatch/verification is the next experiment.
- Only one DeepSeek run; plan quality across models is not measured.
- The skill source here is the Ponytail plugin; a lark skill was not installed as a
  `SKILL.md`, so the run does not prove lark-cli execution. That is intentional: the
  planner discovers skills; Workers apply them.
- Image OCR and legacy Office backends are separate; see `docs/document-readers.md`.

## Reproduce

```bash
1agent master plan \
  --task "请你先阅读 docs/agent-registry.md，然后用 skill_search 搜索 ponytail 技能并阅读它的说明，最后为给 skynet-term-agents list/find 增加 --count 的改动生成一个 Work Item 计划；把 ponytail 的约束写进 objective，不要执行。" \
  --config ~/.1a/config.json \
  --workspace /tmp/skynet-workshop-parallel-dev \
  --output plan.json \
  --planner-journal-dir ./journal \
  --json
```

The raw journal is intentionally not committed; `journal.sha256` records its digest.
