# Planner skill execution live run (2026-11-20)

Status: **passed on this machine with the installed binary**
Binary: `~/.local/bin/1agent` (release, sha256 `e1529700…`, built from this worktree)
Model: DeepSeek `deepseek-flash`
Source: Lark/Feishu DOCX `HztBdLEAXorDhQxCYp1cyDXOnmd` (revision 8)
Workspace: `/home/skynet/1-agent`
Plan: 3 Work Items, `approved=false`, all `pending`

## What was tested

`1agent master plan` was given:

> 先使用 skill_exec 调用 lark-cli 阅读飞书文档
> `https://szchenqi.feishu.cn/docx/HztBdLEAXorDhQxCYp1cyDXOnmd`，理解其中的
> 90 天落地方案；然后为文档中与 Skyget/1Agent 产品化相关的工作生成 2 到 3 个
> Work Item 的执行计划，把文档里的关键约束写进 objective。不要执行，不要过度
> 浏览仓库，读完文档和必要上下文就返回计划。

The planner had `read_file`, `search_files`, `read_document`, `skill_search` and
`skill_exec`. The operator policy `~/.1a/skill-exec.json` allowed only
`lark-cli skills list`, `skills read`, `docs +fetch` and `docs +search`;
`--allow-network` was required and passed.

## Result

- Step 0: `skill_exec skills list`; local `search_files`.
- Step 1: `skill_exec skills read lark-doc`; read `README.md`.
- Step 2: a fetch with `--doc-url` failed (exit 2, unknown flag); the planner
  corrected to `--doc` without user help.
- Step 3: `skill_exec docs +fetch --doc <url> --as user` returned `ok:true`,
  `document_id=HztBdLEAXorDhQxCYp1cyDXOnmd`, `revision_id=8`, and 24,270 chars
  of tool content (not truncated).
- Step 4: local `search_files` for repository context.
- Step 5: returned a valid 3-Work-Item PlanProposal. Its summary explicitly says
  it read the document with `lark-cli`, cites the document ID and revision, and
  records Chapter 7's "1-Agent 是观察、不投入、不写文档、不推广" boundary.
- Every tool call and observation is in the planning journal; `journal.sha256`
  records its digest. The raw journal and full plan stay local and are not
  committed.

Work Items:

- `skyget-90day-brief` — Skyget 90 天落地方案执行摘要与工作分解
- `1agent-skyget-boundary-memo` — 1Agent 与 Skyget 的边界与投入度备忘（内部）
- `skyget-gate-and-spec-checklist` — Skyget 四道门禁检查单与埋点/自愈/license 规格草案

## What this proves

- The planner can discover a CLI's embedded skill, read its instructions, and
  fetch a private Feishu document through `skill_exec`.
- The boundary held: no shell, only declared read prefixes, network gated by
  `--allow-network`.
- A failed tool call becomes an observation; the planner corrected the flag
  without user help.
- The plan is grounded in the fetched document rather than model prior.
- The installed release binary, not only `target/debug`, serves the capability.

## What this does not prove

- The plan was not approved or executed.
- One DeepSeek run; plan quality across models is not measured.
- The document content and full plan are not committed; only metadata, hashes
  and a summary are in this directory.
- `skill_exec` is wired into the `master plan` CLI, not the TUI production
  planner.
- "Read-only" means the declared command prefixes; the CLI may refresh its own
  auth cache.

## Tests

- `cargo test --lib -- skill_exec master_planner`: 18 passed / 0 failed.
- `cargo test --lib`: 1301 passed / 0 failed / 4 ignored.
- `cargo test --test tool_search_cli --test tools_e2e`: 3 passed / 0 failed.
- `cargo clippy --lib`: no warnings.

## Reproduce

```bash
1agent master plan \
  --task "先使用 skill_exec 调用 lark-cli 阅读飞书文档 <doc-url>，理解其中的 90 天落地方案；然后生成 2 到 3 个 Work Item 的执行计划，把关键约束写进 objective。不要执行。" \
  --config ~/.1a/config.json \
  --workspace /home/skynet/1-agent \
  --allow-network \
  --output plan.json \
  --planner-journal-dir ./journal \
  --json
```

`lark-cli` must be installed and authenticated, and `~/.1a/skill-exec.json` must
contain the read prefixes shown in `docs/skill-exec.md`.
