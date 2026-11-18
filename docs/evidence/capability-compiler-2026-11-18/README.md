# Capability compiler smoke (2026-11-18)

Scope: validate that the model actually uses the new P0 capability compiler on
an artifact task. This is a behavior smoke, not a pass gate.

## Command

```bash
1agent run --config ~/.1a/config.json \
  --task "画一张架构图，展示任务理解、能力解析、工具选择和产物验收四个阶段" \
  --journal-dir <tmp> --json
```

## Observed tool sequence

```text
resolve_capability
capability_search
write_file
preview_file
browser
browser
acceptance_gate
```

- `resolve_capability` returned a named gap: no installed `render` capability
  for `image/svg+xml`; `archify` was listed as an uninstalled alternative.
- The model then built an SVG manually, wrote it, previewed it, ran a browser
  audit, and `acceptance_gate` accepted 5 criteria.
- The final answer was `needs_evidence` because the default step budget was
  exhausted before the final answer. Artifact:
  `architecture-4-stages.svg` (SHA-256
  `a404d3664cf710582d5cbcb2b162213af151d9bdec8b62f4d8dd7e63cc860096`).

## Interpretation

The mechanism is now reachable: the model called `resolve_capability` and saw
the gap and alternatives instead of inventing a tool name. The run also exposes
the next problem: a valid artifact + accepted receipt can still end in
`needs_evidence` because the step budget is consumed before the final answer.
That belongs to the P1 runtime/finalization work, not the resolver.

Raw: `answer.json`, `tool-sequence.json`, `observations.json`,
`architecture-4-stages.svg`.
