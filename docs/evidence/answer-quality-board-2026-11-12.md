# Answer-Quality Board (M19–M23) — Evidence (2026-11-12)

Status: **passed on this machine**
Scope: `AnswerQualitySample`, `summarize_answer_quality` and
`1agent answer-quality`.

## Commands

```bash
cargo test answer_quality --lib
cargo test --test answer_quality_cli
```

## Example

Input samples:

```json
{"first_answer_usable":true,"pairwise_win":true}
{"first_answer_usable":true,"pairwise_win":false,"over_structured":true,"unnecessary_followup":true,"followup":true,"followup_adopted":true}
{"first_answer_usable":false,"followup":true}
{"first_answer_usable":true}
```

Output:

```text
samples=4 M19_usable=75.00% M20_win=50.00% (compared=2)
M21_over_structured=25.00% M22_unnecessary_followup=25.00%
M23_followup_adopted=50.00% (followups=2)
```

## What this proves

- M19–M23 are computed deterministically from the six sample fields.
- Ties (`pairwise_win: null`) are excluded from M20; a tie does not count as
  a loss.
- Empty samples produce `null` rates instead of a fake percentage.
- A malformed JSONL line reports its line number.
- `followup_adopted` without `followup` fails closed.
- The CLI exposes JSON and a compact human summary.

## What this does not prove

- That a blind review was conducted, or that the labels are correct.
- M19–M23 targets; no golden set or Hermes baseline is included here.
- M1–M18; those come from the journal metrics command and other sources.
- Inter-rater agreement or annotation calibration; those are separate
  annotation workflows.
