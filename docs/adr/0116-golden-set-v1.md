# ADR 0116: Golden-Set Prompt Inventory v1

- Status: accepted
- Date: 2026-11-12
- Related: SA97-32, M19–M23

## Context

SA97-32 requires 30–50 fixed prompts spanning chat, fact, decision, research
and execution. The M19–M23 board already consumes reviewer labels, but there
was no versioned prompt inventory, so different review runs could silently use
different questions. The golden set had to be fixed before any blind review.

## Decision

1. `evals/golden-set-v1.jsonl` contains 40 prompts: eight per category.
   Each line is a `GoldenPrompt` with `id`, `category`, `form`, `prompt` and
   optional `notes`.
2. Categories are `chat`, `fact`, `decision`, `research` and `execution`.
   Forms are `direct`, `decision`, `research` and `execution`.
3. `golden_set::parse_golden_set` rejects malformed JSONL with a line number,
   duplicate ids or prompts, ids outside 64 safe ASCII characters, empty or
   over-2,000-character prompts, control characters, a count outside 30–50,
   and any missing category.
4. `1agent golden-check --input PATH [--json]` validates the inventory and
   reports counts by category and form. It does not run a model or create
   labels.
5. Changing the prompt set requires a new ADR/CHANGELOG entry, consistent with
   the anti-cheating rule for benchmarks.

## Consequences

- M19–M23 review runs can now cite a fixed, versioned prompt set.
- This is inventory validation, not evidence of answer quality. Blind review,
  reference answers and labels are still required.
- The set is model-agnostic and provider-agnostic; no API calls are made.
- Prompt changes after labels exist must be versioned rather than edited in
  place, or prior labels become incomparable.
