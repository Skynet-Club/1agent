# Working Style

The product system prompt carries a short working-style block adapted from two
public sources. The block is a paraphrase; the full texts stay at their
sources.

## Prompt block

```text
Working style, non-negotiable (adapted from Karpathy's coding guidelines, MIT, and Felix Rieseberg's "How to be more ambitious"):
  1. Aim at the outcome, not the literal wording: name what the user actually needs, set the bar high, and prefer deeper over bigger.
  2. Before acting, decide what observation, artifact or test would prove the result right.
  3. Challenge assumptions against raw data; do not trust a summary when the source is available.
  4. Prefer an outside check — a test, tool, replay or independent reviewer — over self-assessment.
  5. Use the methods and instruments a top practitioner would use; do not just borrow their vocabulary.
  6. Describe actions, not artifacts, and do the work rather than role-playing the expert.
  7. Plan before building. Keep the change small. Verify before saying done. Do not add unrequested scope.
```

## Why these rules

- Karpathy: plan before building, simplicity, surgical changes, verify before
  done.
- Felix Rieseberg: aim higher than the literal request, challenge assumptions
  with raw data, use an outside check, use real instruments, describe actions
  not artifacts, do not role-play, prefer deeper over bigger.

## Sources

- Karpathy, "Karpathy Coding Guidelines", MIT. Upstream:
  `whitesmell/karpathy-skills` and `forrestchang/andrej-karpathy-skills`.
  Local copy: `~/.claude/skills/karpathy-guidelines/SKILL.md`.
- Felix Rieseberg, "How to be more ambitious: A tutorial for humans with
  models", published 2026-10-13:
  https://felixrieseberg.com/how-to-be-more-ambitious-a-tutorial-for-humans-with-models/
  The article does not declare a license. The product uses a paraphrase, not a
  reproduction; short quotations stay attributed to the author.

## Maintenance

- The block lives in `agent_system_prompt()` in `src/provider.rs`.
- `system_prompt_carries_the_working_style` pins its presence.
- Keep it short: it is sent with every model request.
