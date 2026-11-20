# Planner source gate (2026-11-20)

Status: **passed for URL and local sources on this machine**

## Why this gate exists

- **Scenario**: a `master plan` task names an external requirement source — a
  Feishu/Lark URL, a local requirements file, a directory, or a third-party
  knowledge-base export (Obsidian is one example, not a special case).
- **Who**: the operator using `master plan` (and later the TUI planning entry).
- **Problem**: deterministic plan validation checks structure, IDs and
  permissions, but not whether the model actually read the source. A local 8B
  produced a schema-valid, entirely fabricated multi-cloud/SOC2 plan without
  reading the Feishu document.
- **Fix**: a plan is rejected with `UnreadSource` unless a successful tool call
  covered every named source. This turns silent hallucination into a clear
  error. It proves "was read", not "was used"; read-then-ignore is still
  possible.

## Source rules

| Source | Declaration | Required successful read |
|---|---|---|
| `http(s)` URL in the task | automatic | `skill_exec` call containing the URL |
| Local file | `--source <path>` | `read_file` or `read_document` on the file |
| Local directory / knowledge base | `--source <dir>` | a read or `search_files` under the directory |
| URL also declared | `--source <url>` | same as URL in the task |

`--source` is repeatable. A local source also becomes a planner read root: a
file grants its parent directory, a directory grants itself. Work Items remain
inside `context.workspace`; external sources are read by the planner, and the
relevant facts go into the Work Item objective. This avoids silently widening
Worker permissions.

Example:

```bash
1agent master plan \
  --task "先阅读需求文档和知识库，然后生成 2 个 Work Item 的计划；不要执行。" \
  --source /home/skynet/文档/需求.md \
  --source /home/skynet/文档/Skynet-Docs/Skynet-Club \
  --config ~/.1a/config.json \
  --workspace /home/skynet/1-agent \
  --output plan.json \
  --planner-journal-dir ./journal \
  --json
```

## Live results

| Run | Binary | Result |
|---|---|---|
| DeepSeek URL source | `d620eb9a…` | passed: read the Feishu DOCX (revision 8), 3 grounded Work Items, `approved=false` |
| DeepSeek local file source | `d620eb9a…` | passed: `read_file` on `/tmp/.../requirements.md` (288 B), 2 Work Items, workspace-only read roots |
| Local `qwen3-8b-16k` URL source | `15322184…` | rejected: `UnreadSource`, exit 1, no plan written |

Metrics:

| Run | Requests | Tool calls | Elapsed | Input/output tokens |
|---|---:|---:|---:|---:|
| DeepSeek URL | 10 | 12 | 53.6 s | 190,998 / 8,768 |
| DeepSeek local file | 4 | 5 | 13.3 s | 14,474 / 2,358 |
| Local 8B URL | 4 | 3 | 196.2 s | 7,539 / 4,654 |

## What this proves

- The URL gate blocks a model that never read the named document.
- `--source` lets the planner read a local file/directory outside the workspace
  with the existing `read_file` / `read_document` / `search_files` tools.
- A directory source is accepted after a successful `search_files` under it,
  which covers third-party knowledge-base exports generally.
- DeepSeek still passes both URL and local-source tasks with grounded plans.

## What this does not prove

- One run per path; no repeated sampling.
- The gate checks a successful read, not semantic use. A model can read and
  then still fabricate.
- Work Items do not inherit external source read roots; the planner must put
  the relevant facts into the objective.
- Local 8B quality is not a release gate; its rejection is the intended
  fail-closed behavior.

## Tests

- `cargo test --lib -- master_planner skill_exec`: 26 passed / 0 failed.
- `cargo test --lib`: 1309 passed / 0 failed / 4 ignored.
- `cargo clippy --lib`: no warnings.

## Evidence files

- `url-deepseek-plan-summary.json` + `url-deepseek-*.sha256`
- `url-ollama-gate-output.txt` + `url-ollama-journal.sha256`
- `local-source-requirements.md`, `local-source-plan.json`,
  `local-source-tool-trace.txt`, `local-source-*.sha256`

Raw journals and the full URL-grounded plan stay local; only summaries and
hashes are committed.
