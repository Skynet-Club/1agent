# Clarify Gate — Evidence (2026-11-12)

Status: **passed on this machine**
Scope: `ClarifyQuestion`, `ClarifyGate`, normalization and the `clarify` CLI.

## Commands

```bash
cargo test clarify --lib
cargo test --test clarify_cli
```

## What this proves

- Normalization adds the automatic `other` option and moves the recommended
  option first.
- The gate admits one question per turn by default, deduplicates ids and
  case/whitespace-insensitive prompts, and admits the next unseen question on
  a later turn.
- A one-question total budget admits once and skips afterwards; duplicates do
  not consume the budget.
- A batch with more than five blocking questions is rejected.
- Few-zero options, duplicate option ids, multiple recommended options and
  control characters in the prompt fail closed.
- The CLI accepts an array or `{"questions":[...]}`, prints `ask` with the
  normalized question and a 300-second timeout, prints `skip` for an empty
  batch, and rejects invalid questions and oversized blocking batches with a
  non-zero exit.
- `resolve_round` counts answered/skipped/unanswered, treats missing answers
  as unanswered, counts blocking questions without an answer, and keeps
  `default_answer_valid` true. Unknown questions/options, duplicate answers,
  answered-without-option, skipped-with-option and an empty submission fail
  closed.
- The CLI resolves an answers file against the admitted questions and prints
  the summary for `submitted` and `cancelled` rounds.

## What this does not prove

- That a TUI or any other frontend renders the questions; no panel exists.
- That answers are persisted or survive the process; the CLI is one-shot.
- Deduplication across sessions or processes; the gate is per-process.
- That a blocking question actually blocks for 300 seconds; only the timeout
  value is emitted.
