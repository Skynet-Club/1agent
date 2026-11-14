# Blind Review Packet — M19–M23 (2026-11-12)

Status: **pipeline ready; packet collection scheduled after the 8h benchmark**
(`job-1794497599059-2245-3`). A human reviewer is still required.

## Files

- `prompts.jsonl`: 40 fixed golden-set prompts, each with two anonymized
  answers `answer_a` and `answer_b`. The A/B mapping is sealed outside this
  directory.
- `labels-template.jsonl`: one row per answer (80 rows) with empty labels.
- Answers and the sealed mapping live under `~/.1a/blind-review/`, not in this
  packet.

## Reviewer instructions

For each row, read the prompt and both answers, then fill the label row for
`side: "a"` and `side: "b"`:

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

Do not open the sealed mapping until all 80 rows are labeled. The labels are
the external evidence; this packet is not the result.

## Finalize

After filling the template:

```bash
python3 scripts/blind-review.py finalize \
  --labels labels.jsonl --out answer-quality.jsonl
1agent answer-quality --input answer-quality.jsonl --json
```

The output reports M19–M23; zero denominators are `null`. Report the sample
count and the sealed-mapping hash with the result.

## Collection command (run after the benchmark)

```bash
python3 scripts/blind-review.py collect --runner 1agent --out ~/.1a/blind-review/answers-1agent.jsonl
python3 scripts/blind-review.py collect --runner hermes --out ~/.1a/blind-review/answers-hermes.jsonl
python3 scripts/blind-review.py packet \
  --oneagent ~/.1a/blind-review/answers-1agent.jsonl \
  --baseline ~/.1a/blind-review/answers-hermes.jsonl \
  --out-dir docs/evidence/blind-review-2026-11-12 \
  --mapping ~/.1a/blind-review/mapping-2026-11-12.json
```

1Agent runs read-only and Hermes runs `--safe-mode`; the comparison is answer
quality, not action capability.
