# Planner skill_exec cross-model comparison (2026-11-20)

Status: **completed; DeepSeek passed, local qwen3-8b-16k failed the doc-grounded task**

Note: the source gate added after this run now rejects the 8B's plan with
`UnreadSource`; see `../planner-source-gate-2026-11-20/`.

Same task, same workspace, same `~/.1a/skill-exec.json` policy, same installed
binary (`~/.local/bin/1agent`, sha256 `e1529700…`). Only the provider/model
changed:

| Run | Provider | Model | Config |
|---|---|---|---|
| DeepSeek | `deepseek` | `deepseek-flash` | `~/.1a/config.json` |
| Ollama | `ollama-local` | `qwen3-8b-16k` (num_ctx 16384) | `configs/ollama-local.json` |

Task:

> 先使用 skill_exec 调用 lark-cli 阅读飞书文档
> `https://szchenqi.feishu.cn/docx/HztBdLEAXorDhQxCYp1cyDXOnmd`，理解其中的
> 90 天落地方案；然后为文档中与 Skyget/1Agent 产品化相关的工作生成 2 到 3 个
> Work Item 的执行计划，把文档里的关键约束写进 objective。不要执行，不要过度
> 浏览仓库，读完文档和必要上下文就返回计划。

## Results

| Metric | DeepSeek `deepseek-flash` | Ollama `qwen3-8b-16k` |
|---|---|---|
| Model requests | 6 | 9 |
| Tool calls | 8 | 5 |
| Successful doc reads | **1** (`docs +fetch`, 24,270 chars, revision 8) | **0** |
| Failed tool calls | 1 (`--doc-url`, corrected to `--doc`) | 4 (2 invalid `skill_exec`, 2 missing local PDF) |
| Schema-valid plan | yes | yes |
| Work Items | 3 | 3 |
| Document-grounded | yes (positioning sentence, gates 1–4, Chapter 7 non-goal) | no (invented 99.95% SLA, SOC2, Grafana/Prometheus) |
| Boundary compliance | yes (1Agent non-goal respected; no network) | no (invented multi-cloud scope; `allow_network: true`) |
| Elapsed | 43.1 s | 198.1 s |
| Input / output tokens | 94,953 / 8,132 | 18,845 / 7,292 |
| Rubric score | **10 / 10** | **1 / 10** |

## Rubric

| Dimension | Weight | DeepSeek | Ollama |
|---|---:|---:|---:|
| Discovered the CLI's embedded skill (`skills list` / `skills read`) | 1 | 1 | 0 |
| Fetched the Feishu document via `lark-cli docs +fetch` | 2 | 2 | 0 |
| Recovered from a bad tool call without user help | 1 | 1 | 0 |
| Returned a schema-valid PlanProposal | 1 | 1 | 1 |
| Plan grounded in at least three document-specific constraints | 3 | 3 | 0 |
| No fabricated document claims | 1 | 1 | 0 |
| Preserved document scope/boundaries (1Agent is a non-goal) | 1 | 1 | 0 |
| **Total** | **10** | **10** | **1** |

## What the local 8B actually did

- `skill_search lark-cli` found 0 skills (the CLI's embedded skill is not a
  filesystem `SKILL.md`).
- It tried `skill_exec ["fetch", <url>]` and `["docs", "fetch", <url>]`; both
  were rejected because the allowlist requires `docs +fetch`.
- It then invented a local path `docx/HztBdLEAXorDhQxCYp1cyDXOnmd.pdf`,
  first with an over-limit `max_chars` and then as a missing file.
- It never read the document. The final plan is schema-valid but generic
  multi-cloud/security/observability work, with fabricated "per document"
  claims and `allow_network: true`.
- It did not exhaust the 12-step / 24-call budget; it stopped after 9 requests
  and 5 tool calls.

## What this proves

- The `skill_exec` mechanism works for both providers: both models could see
  the tool, call it, receive structured observations and return a plan.
- Model choice still determines whether the plan is grounded. The deterministic
  plan validator checks structure and identity, not whether the model read the
  source.
- Tool-error observations are not self-correcting for a small model: the 8B
  repeated a wrong shape and then fabricated a local file path.

## What this does not prove

- One run per model; no repeated sampling or confidence interval.
- `qwen3-8b-16k` runs with a 16k context; the DeepSeek run used much more input
  context. This is a capability comparison, not an equal-context benchmark.
- The prompt and tool schema were written for a capable tool-calling model; a
  smaller model may need a shorter, more explicit recipe.
- Local 8B quality is not a release gate; the product claim is about mechanism
  and evidence, not local-model quality.

## Evidence

- DeepSeek installed-binary run: `../planner-skill-exec-2026-11-20/`
  (journal digest `b4226c07…`, plan digest `d4b93f47…`).
- Ollama journal digest: `ollama-journal.sha256`.
- Ollama plan: `ollama-plan.json` (schema-valid but not document-grounded);
  digest in `ollama-plan.sha256`.
- Machine-readable metrics: `metrics.json`; tool trace: `ollama-tool-trace.txt`.

## Reproduce

DeepSeek:

```bash
1agent master plan --task "<task above>" \
  --config ~/.1a/config.json \
  --workspace /home/skynet/1-agent --allow-network \
  --output /tmp/skill-exec-plan.json \
  --planner-journal-dir /tmp/skill-exec-plan-journal --json
```

Ollama:

```bash
OLLAMA_MODELS=$HOME/.local/share/1agent-ollama/models \
OLLAMA_HOST=127.0.0.1:11434 \
  $HOME/.local/share/1agent-ollama/dist/bin/ollama serve &

1agent master plan --task "<task above>" \
  --config /home/skynet/1-agent/configs/ollama-local.json \
  --workspace /home/skynet/1-agent --allow-network \
  --output /tmp/skill-exec-ollama.json \
  --planner-journal-dir /tmp/skill-exec-ollama-journal --json
```
