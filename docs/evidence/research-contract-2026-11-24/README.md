# Research answer status contract — 2026-11-24

`research-compare` asks which deployment option is preferable from two
one-line files: A has lower total cost, B has lower migration risk. The suite
expects `conditional`, because the sources exist but neither option wins
without the user's weights or missing magnitudes. Earlier runs returned
`completed` (an unresolved trade-off presented as settled) or `needs_evidence`
(as if a source were missing); both violate the contract.

## Contract

The core system prompt now states:

- `needs_evidence` means a required source is missing or unreadable;
- if sources exist but the winner depends on user weights or missing
  magnitudes, return `conditional` with a deciding flip condition;
- do not use `completed` for a mere trade-off.

The prompt budget (`prompt_budgets`) requires the answer-style delta to stay
under 2,800 tokens, so redundant wording elsewhere in the core prompt was
compressed without removing rules. `verify.sh` is green; lib **1456/0/4**, bin
**73/0**, core eval 8/8.

## Tests

- Prompt contract assertions in
  `system_prompt_defines_model_prior_source_rules`.
- `prompt_budgets` stays within all three budgets.
- Focused `research-compare` eval, binary `269a6b7e…` (long-form rule):
  **N=3 3/3** and **N=5 5/5**.
- Focused `research-compare` eval, binary `b24040a7…` (compressed prompt):
  **N=5 5/5**.
- Full 24-case eval, binary `b24040a7…`: **24/24 (100%), pass^3 8/8,
  at least once 8/8**, no failure classes. The answer in the sampled journal
  is `conditional` with both `a.md` and `b.md` as evidence.

## Evidence

- `research-suite.json`, `report.json`, `report-n5.json`,
  `report-trim-n5.json`, `report-full.json`.
- `stderr*.log` — empty.
- `product-eval*/` — per-repeat journals; ignored by Git.

## Binary

Final installed `~/.local/bin/1agent` sha256 `b24040a7…`; previous
`5ef5fb7c…` saved as `~/.local/bin/1agent-pre-research-contract-2026-11-24`.
