# ADR 0103: Answer-Quality Board (M19–M23)

- Status: accepted
- Date: 2026-11-12
- Related: M19–M23 metric table, SA97-33

## Context

M19–M23 depend on blind review judgments (first-answer usability, pairwise
win rate, over-structure, unnecessary follow-up, follow-up adoption). There
was no deterministic projection from review samples to those rates, so every
review risked a hand-computed number.

## Decision

1. Define `AnswerQualitySample` with six fields:
   `first_answer_usable`, `pairwise_win` (`true` win, `false` loss, `null`
   tie/not compared), `over_structured`, `unnecessary_followup`, `followup`
   and `followup_adopted`.
2. `summarize_answer_quality` computes:
   - M19 = usable / samples;
   - M20 = wins / (wins + losses), ties excluded;
   - M21 = over-structured / samples;
   - M22 = unnecessary follow-up / samples;
   - M23 = adopted / follow-ups.
   All rates are basis points and `null` when the denominator is zero.
3. `parse_answer_quality_samples` reads JSONL, reports the source line on a
   malformed record, and rejects `followup_adopted` without `followup`.
4. `1agent answer-quality --input PATH [--json]` exposes the board. The input
   is capped at 16 MiB and must be UTF-8.

## Consequences

- Reviewers can record judgments in a simple JSONL file and get the five
  rates deterministically; the same samples always produce the same report.
- The command does not run a blind review, judge answers, or assemble the
  golden set. It only computes rates from caller-supplied labels.
- A tie is not a loss; excluding ties is explicit so the win rate is not
  diluted by “no comparison”.
- The labels are self- or human-supplied and can be wrong; the report is not
  evidence of answer quality without a proper blind protocol.
