# Blind Review Packet — M19–M23 (2026-11-13)

Status: **packet ready; human labels pending**

- 39 prompts × 2 sides = 78 label rows.
- `research-05` is excluded: the Hermes `--safe-mode` baseline timed out on that
  prompt even with a 300s timeout. The packet is explicit about the reduced
  sample.
- The A/B mapping is sealed at
  `~/.1a/blind-review/mapping-2026-11-13.json` and is not in this directory.

## Files

- `prompts.jsonl`: the fixed golden-set prompts with anonymized `answer_a` and
  `answer_b`.
- `labels-template.jsonl`: one empty label row per answer (78 rows).
- Answers and the sealed mapping live under `~/.1a/blind-review/`, not in this
  packet.

## Reviewer instructions

For each row, read the prompt and both answers, then fill both label rows:

- `first_answer_usable` (M19): true if the answer can be used without a
  follow-up correction.
- `pairwise_win` (M20): true if this side is better, false if worse, null for a
  tie or no comparison.
- `over_structured` (M21): true if formatting is heavier than the question
  needs.
- `unnecessary_followup` (M22): true if the answer asks for information it
  should have inferred or already had.
- `followup` (M23): true if the answer asks a question or proposes one.
- `followup_adopted` (M23): true if the suggested follow-up was actually
  needed/adopted; false otherwise; null when `followup` is false.

Do not open the sealed mapping until all rows are labeled. The labels are the
external evidence; this packet is not the result.

## Finalize

```bash
python3 scripts/blind-review.py finalize \
  --labels labels.jsonl --out answer-quality.jsonl
1agent answer-quality --input answer-quality.jsonl --json
```

## Collection notes

1Agent ran read-only; Hermes ran `--safe-mode --provider deepseek --model
deepseek-flash`. Safe mode ignores the user profile, so both flags are
required. One prompt timed out on the Hermes side and was excluded rather than
substituted.
