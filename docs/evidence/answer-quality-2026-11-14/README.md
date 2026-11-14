# Answer-quality blind packet (2026-11-14)

Status: **labels adopted by the product owner, but not a human blind review**

## What was measured

- `evals/golden-set-v1.jsonl`: 40 prompts, 8 each in chat/fact/decision/research/execution.
- Four answerers: 1Agent, Hermes, Codex, OpenCode. Each got the same prompt.
- Answers were anonymized per prompt into A/B/C/D with a SHA-256-seeded shuffle;
  obvious product names and tool-specific home paths were neutralized in the
  packet. The sealed side-to-runner mapping is `mapping.json`.
- 160/160 answers were collected; two Hermes research prompts timed out at 240 s
  and were retried with a longer timeout or a no-tools fallback.

## Labels and provenance

The Feishu Bitable `盲评-简版` was filled twice:

1. First pass: `标注人=hermes-agent`, 37/40 first choices were side D; all 40
   records were updated in bulk. This pass was discarded.
2. Second pass: `标注人=我`, 40/40 `已标注`, first choices A 11 / B 12 / C 7 /
   D 9 / tie 1. The 40 records were written in the same two-second window
   (`1794664686`–`1794664688`), so this is also a bulk/API write, not
   row-by-row human labeling.

The product owner reviewed the finding and chose to **keep and adopt** the
second pass. It is therefore recorded as **AI-assisted / bulk labels**, not as
the human blind-review gate. Hermes is one of the four answerers and wins
26/39 rows in this pass; directional results should be read with that conflict
of interest in mind.

## Results (1Agent vs the field)

| Metric | Value | Notes |
|---|---|---|
| M19 first-answer usable | **85.00%** (34/40) | 6 rows marked 首答不可用 |
| M20 best-of-4 win rate | **5.13%** (2/39) | wins chat-04, execution-02; 1 tie excluded |
| M21 over-structured | **2.50%** (1/40) | |
| M22 unnecessary follow-up | **0.00%** (0/40) | |
| M23 follow-up adoption | **null** | no follow-up was labeled in this packet |

Wins by runner (side-randomized, 1 tie excluded): Hermes 26, OpenCode 7,
Codex 4, 1Agent 2.

By category, 1Agent: chat 1 win / 6 losses / 1 tie; fact 0/8; decision 0/8;
research 0/8; execution 1/7.

## Files

- `samples.jsonl`: 40 `answer-quality` samples for 1Agent.
- `summary.json`: full report, per-side win counts and per-row detail.
- `feishu-labels.json`: raw `record-list --format json` export used to compute
  the samples.
- `packet.jsonl`: the anonymized A/B/C/D packet shown to the labeler.
- `mapping.json`: sealed side-to-runner mapping, unsealed for this archive.
- `SHA256SUMS`: hashes of the files above.

## Reproduction

```bash
cargo run --offline --quiet -- answer-quality \
  --input docs/evidence/answer-quality-2026-11-14/samples.jsonl --json
```

The metric board only computes rates from the supplied labels; it does not
validate who wrote them.
