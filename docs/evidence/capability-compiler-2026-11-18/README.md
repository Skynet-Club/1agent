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

## rc.10 finalization smoke

After the step-budget finalization fix, the same 6-step run now ends with a
final answer instead of the canned budget answer:

- status: `conditional` (the answer names the unverified font fallback)
- stop reason: `completed`
- artifact: SVG + HTML written, `preview_file` HTTP 200, browser audit
  390/768/1440 with 0 overflow / 0 placeholders / 0 small tap targets,
  `acceptance_gate` accepted.
- tool sequence: `resolve_capability → capability_search → write_file ×2 →
  preview_file → browser ×2 → acceptance_gate`.

Raw: `rc10-finalization/answer.json`, `summary.json`, `tool-sequence.json`.

## rc.12 final-answer and path-memory smoke

Two 12-step runs with the real provider config:

1. `rc12-finalization/`: final answer `completed`; `write_file` +
   `verify_artifact` + `acceptance_gate` accepted; no repair failure. This run
   was built before the preflight journal event, so it recorded no capability
   path.
2. `rc12-preflight-memory/`: final answer `completed`; verified SVG and
   accepted receipt; `~/.1a/capability-paths.jsonl` now contains:

```json
{"schema_version":1,"requirement":{"verb":"render","inputs":["structured_spec"],"outputs":["image/svg+xml"],"constraints":["local"],"required":true},"manifest":"manual-svg","status":"completed","artifact_verified":true}
```

The deterministic test `accepted_receipt_survives_unrepairable_plain_text_final`
covers the repair-failure fallback; the live model returned valid JSON this
time, so that fallback was not exercised live.
