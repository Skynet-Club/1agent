# Planner typed source entry (`read_source`) live run (2026-11-20)

Status: **typed entry works for DeepSeek URL/local and for local 8B reading;
the 8B still fails the plan wrapper/identity contract**

## What changed

The planner now has `read_source`: the model names a source, the runtime picks
the backend. The model no longer composes `lark-cli` flags or reads the CLI's
skill documentation first.

| Source | Runtime backend |
|---|---|
| Lark/Feishu/Doubao `/docx/` or `/wiki/` URL | `lark-cli docs +fetch --doc <url> --doc-format markdown` |
| other public `https://` URL | `fetch_url` |
| local file | `read_document` |
| local directory / knowledge base | `search_files` (with `query`) |

Security: no shell; fixed Lark argv; remote reads require `--allow-network`;
local reads stay inside `read_roots`; `max_chars` is clamped to 1,000..200,000;
outputs and subprocess environment are bounded.

## Live results

| Run | Binary | Result |
|---|---|---|
| DeepSeek + Feishu DOCX | `69233386…` | passed: one `read_source` call read revision 8; 3 grounded Work Items; 42.4 s |
| DeepSeek + local file/directory | `69233386…` | passed: `read_source` file + directory search; 2 Work Items; 26.1 s |
| Local `qwen3-8b-16k` + Feishu DOCX | `c14971c8…` | read succeeded at step 1; plan wrapper/identity failed repeatedly; no plan |

The local 8B's first `read_source` used `max_chars=1000000` and was rejected;
the second call used 200000 and **successfully read the Lark document**. The
final binary clamps `max_chars` to 200000, so that first error would disappear
in a rerun. The remaining 8B failure is not reading: it returns a bare
`PlanProposal` (`unknown field plan_id`) or an `AgentAnswer` whose `conclusion`
is an object instead of a JSON string.

## What this proves

- A typed entry lets the model express **what to read** while the runtime owns
  **how to read**.
- DeepSeek moves from 12 tool calls / 190,998 input tokens on the skill_exec
  flow to 6 calls / 49,128 input tokens on `read_source`, with the same
  document-grounded plan quality.
- Local 8B can use the typed entry to read a Feishu document; the runtime
  handles lark-cli, auth and argument composition.
- The source gate accepts `read_source` as a successful read.

## What this does not prove

- Local 8B cannot yet produce a schema-valid plan on this task. That is a
  plan-contract/model capability gap, not a reading-entry gap.
- Public HTTPS `fetch_url` was not live-tested here; it is unit/backend covered.
- Lark support requires an installed, authenticated `lark-cli`.
- The full Feishu-grounded plan stays local; only the summary and hashes are
  committed.

## Next step for the 8B

Two harness-level options, to be decided separately:

1. Structured output / JSON mode for the plan response, so the model emits the
   `AgentAnswer` wrapper and string `conclusion` correctly.
2. A bounded wrapper repair: accept a bare `PlanProposal` and normalize
   code-owned `plan_id`/`goal_id`/`conclusion` before validation, while keeping
   content and permission checks strict.

## Tests

- `cargo test --lib`: **1316 passed / 0 failed / 4 ignored**.
- `read_source`: 6 tests (local file, directory search, read-root denial,
  network/lark requirement, fake lark-cli parse, URL classification).
- `cargo clippy --lib`: no warnings.

## Evidence files

- `deepseek-feishu-plan-summary.json`, `deepseek-feishu-*.sha256`
- `deepseek-local-plan.json`, `deepseek-local-*.sha256`
- `tool-traces.txt` (DeepSeek URL/local and 8B traces)
- `ollama-journal.sha256`

Raw journals and the full Feishu-grounded plan stay local.
