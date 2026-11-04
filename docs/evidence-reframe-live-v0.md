# Evidence — Reframe via provider (v0)

Date: 2026-11-04
Model: `deepseek-flash`, one call, temperature default
Status: smoke evidence, not a measurement

## 1. The prompt is now one artefact

`prompts/reframe-v3.txt` is read by both sides:

- Rust: `include_str!` in `src/reframe.rs`, exposed as `REFRAME_SYSTEM_PROMPT`
- Python: `scripts/reframing/run_pilot.py` reads the same file

```text
sha256 650588186c30dffed4ccc6bf2a9e7e601f8871eff7b2e84a9d7f80aecb069e16
```

That is the hash recorded for the v3 held-out round, and a unit test now pins it.
This closes the gap found earlier: the measured prompt and the shipped prompt
were two different strings.

## 2. One live call

```bash
reframe --task "我要买个电钻" --allow-network --base-url https://api.deepseek.com/v1
```

Result: `classification = means`, one question, three options, three substitutes,
`stop_rule_source = derived`. The menu was rendered and every entry pointed at a
persisted proposal file.

## 3. A defect found and fixed in the same pass

The first version printed a menu whose entries re-invoked the model with
`--task ... --accept N`. That would call the model again, produce a different
option list, and make `N` point at a different choice. The proposal is now
written to `<journal-dir>/pending/<id>.json` and the menu references that file,
so the numbers stay valid.

An undecided proposal still writes **no journal**; only the pending file exists.

## 4. Known limitation

The provider trait is answer-shaped: `complete()` parses the reply as an
`AgentAnswer`, so a reframe reply falls through to `InvalidFinal` and the raw
text has to be recovered from there. It works, it is not the right shape. A raw
text method on `ModelProvider` is the proper fix and is not done.

## 5. What this does not show

- One call. No repetition, no different tasks, no failure cases.
- Nothing about whether the question or the substitutes are good. That is the
  semantic round, still blocked on human labels.
- No cost or latency comparison.
