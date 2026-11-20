# Generic planner output normalization (2026-11-20)

Status: **passed; local qwen3-8b-16k now produces a grounded plan without a
model-specific path**

## What changed

The planner output contract is normalized for **all providers**, not a specific
model:

- Accept a plan as:
  - an `AgentAnswer` whose `conclusion` is a JSON string,
  - an `AgentAnswer` whose `conclusion` is an object,
  - a bare `PlanProposal` object.
- Normalize code-owned fields from the planning context:
  - `plan_id`, `goal_id`, `version`.
- Normalize an absolute `expected_artifact` path that is under the Work Item
  workspace into a workspace-relative path; paths outside the workspace remain
  invalid.
- Keep strict content/security validation: self-approval, work-item status,
  workspace/read-root confinement, risk downgrade, artifact rules, schema and
  limits are unchanged.

There is no `if model == 8b` branch. Any OpenAI-compatible/local provider gets
the same parser and the same tool contract.

## Before / after (same task and read_source entry)

| Run | Before (binary c14971c8…) | After (binary 5fda7852…) |
|---|---|---|
| `read_source` | succeeded at step 1 | succeeded in one call (max_chars clamped) |
| Requests / tool calls | 11 / 3 | **2 / 1** |
| Elapsed | 515.6 s (stopped) | **80.7 s** |
| Plan output | repeated `invalid_final`; bare `plan_id` unknown, object `conclusion`, absolute artifacts; no plan | **3 Work Items, approved=false** |
| Grounding | read document, but no plan | positioning/naming, 90-day install/validation, metrics/`job_failed.reason` — matches the document |

The 8B now calls `read_source` with `max_chars=10485760`; the runtime clamps it
and reads the document, then returns a bare `PlanProposal`. The parser
normalizes IDs and absolute artifacts and validates the content.

## DeepSeek regression

Same task with `deepseek-flash` after normalization:

- 7 requests / 5 tool calls / 77.7 s.
- `read_source` for the Feishu document and a local `PRIVACY.md`.
- 3 grounded Work Items, `approved=false`.
- No regression from the normalization.

## What this proves

- A model-agnostic output normalizer is enough to let a weak local model return
  a usable plan when it has read the source.
- Code-owned fields (IDs, version, workspace-relative artifacts) belong to the
  runtime; models should not be rejected for formatting them differently.
- Content and security checks still fail closed.

## What this does not prove

- The 8B's plan is not necessarily high quality; it is structurally valid and
  document-grounded on this task. One run per model.
- Normalization can mask a model formatting error. The raw model response is in
  the planning journal; the final plan is deterministic.
- No public `fetch_url`, no Lark Base/Sheets, no multi-run benchmark.

## Tests

- `cargo test --lib`: **1318 passed / 0 failed / 4 ignored**.
- `master_planner`: 23 tests, including bare plan, object conclusion, identity
  normalization, absolute-artifact normalization and self-approval rejection.
- `read_source`: 6 tests.
- `cargo clippy --lib`: no warnings.

## Evidence files

- `ollama-plan.json`, `ollama-journal.sha256`, `ollama-plan.sha256`
- `deepseek-plan-summary.json`, `deepseek-journal.sha256`, `deepseek-plan.sha256`
- `tool-traces.txt`

Raw journals and the full DeepSeek plan stay local.
