# ADR 0096: Clarify Gate

- Status: accepted
- Date: 2026-11-12

## Context

The agent can ask one free-form question in its answer, but there was no
structured clarify contract: options were not normalized, the frontend could
ask many questions per turn, duplicates were not filtered, and every frontend
would have to invent its own "Other" and recommended-option behavior. The
0.97 plan needs a gate that admits a bounded, deduplicated set of structured
questions and leaves the default answer valid when the user does not answer.

## Decision

1. `ClarifyQuestion` carries an id, a bounded prompt, two to four options and
   a `blocking` flag. Option ids are unique and bounded; at most one option
   may be recommended.
2. Normalization adds an automatic `other` option when absent and moves the
   recommended option first. The automatic option is not a model-provided
   escape hatch that weakens validation.
3. `ClarifyGate` validates the whole batch before admitting anything. It
   filters already-seen ids and case/whitespace-insensitive prompts, admits
   one question per turn by default, and enforces an optional total budget.
   Duplicates do not consume budget.
4. At most five blocking questions may be submitted in one batch
   (`SA97-35`'s batch panel bound). A batch over the limit fails closed.
5. `resolve_round` validates a submitted/cancelled/timed-out/undelivered
   round. Answers must reference admitted questions and existing options; an
   answered question needs an option, a skipped or unanswered one must not
   carry one, and a submitted round needs at least one answer. Missing
   questions count as unanswered, and `default_answer_valid` stays true.
6. `1agent clarify --questions PATH [--answers PATH --outcome STATUS]`
   exposes the gate and round resolution to frontends. It reads an array or
   an object wrapper, rejects input over 1 MiB or more than 50 questions, and
   prints either `ask`/`skip` or the round summary.

## Consequences

- Every frontend sees the same options, with `Other` last and the recommended
  option first, and can report the same five outcome statuses.
- A skipped or unanswered question does not invalidate the default answer;
  the gate only decides what may be asked and how the round is summarized.
- This slice is policy and serialization only. It does not render a panel,
  read stdin, or store answers; the TUI batch panel and persistent round
  history remain SA97-35 work.
- The gate is per-process; deduplication across sessions needs a persisted
  question history and is not claimed here.
