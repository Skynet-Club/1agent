# Golden-Set v1 — Evidence (2026-11-12)

Status: **inventory validated on this machine**
Scope: fixed M19–M23 prompt set and its validator.

## Commands

```bash
cargo test --offline golden --lib
cargo test --offline --test golden_check_cli
target/debug/first-principles-agent golden-check --input evals/golden-set-v1.jsonl --json
```

## Result

- `evals/golden-set-v1.jsonl` has 40 prompts, eight in each of `chat`, `fact`,
  `decision`, `research` and `execution`.
- The shipped set parses and validates.
- Duplicate ids/prompts, a count outside 30–50, a missing category, malformed
  JSONL (with line number), unsafe ids and control characters all fail closed.
- The CLI reports the same counts and exits non-zero for an invalid file.

## What this proves

- There is a fixed, versioned prompt inventory for future blind review.
- The inventory mechanics are deterministic and do not call a model.
- The anti-cheating rule has a concrete artifact to version.

## What this does not prove

- Answer quality, M19–M23 rates or a win against Hermes.
- That any prompt was executed by a real provider.
- Reviewer calibration or label quality.
- That 40 prompts are statistically sufficient; the plan allows 30–50 and this
  is the first version.
