# Compact acceptance gate schema (2026-11-22)

Goal: shrink the `acceptance_gate` schema without weakening validation or
losing the acceptance semantics.

## Change

- The tool description now only states the non-obvious decisions: failed
  hidden criterion -> `hacked`, high-impact residual risk -> `conditional`,
  only `accepted` justifies `completed`. The detailed contract instructions
  already live in `AGENT_PROMPT_ARTIFACTS`, which is sent whenever write or
  acceptance tools are advertised.
- All property descriptions, `additionalProperties: false`, and `default: []`
  were removed from the JSON schema. Types, enums, `required` lists and
  `minItems: 1` are unchanged.
- The Rust argument struct still uses `deny_unknown_fields`; the tool still
  rejects unknown fields, missing evidence and mismatched criteria at
  execution.

`acceptance_gate` spec: 746 -> **379** tokens. TUI core schema:
1,910 -> **1,543**.

## Live check

Installed binary `654ec3de…`, real DeepSeek, TUI. Workspace
`/tmp/tui-accept-compact`, session `ses_tui_accept_compact_20261122`, journal
`c851e21a-0f62-40e0-81fe-72c4dc31228f`.

Task: `Execute: create /tmp/tui-accept-compact/note.txt with exactly the text: compact`

Result: `completed`, file `compact`, 5 model calls / 4 tool calls, 18.1s.

| Metric | Core 8 (`e64e9bc2`) | Compact gate (`c851e21a`) |
|---|---|---|
| Advertised tools | 10 | 10 |
| System tokens | 3,862 | 3,864 |
| Tool schema tokens | 1,910 | **1,543** |
| Fixed system + schema | 5,772 | **5,407** |
| Context tokens | 5,993 | **5,632** |
| Input tokens | 6,215 | **5,868** |

The model filled every acceptance field from the prompt plus the shape-only
schema: 3 public criteria, expected artifact with sha256, artifact check,
independent verifier and all required ids. The first gate call failed with
`criterion has no result` because the model added a criterion and forgot its
result; the clear tool error led to a retry, and the second call returned
`accepted · 3 criteria · verifier read_file full-content read of note.txt`.

## Tests

- `spec_is_shape_only_and_compact`: the acceptance spec is under 400 tokens,
  has 8 required fields and `minItems: 1` on results, and no per-property
  descriptions.
- `core_tools_reduce_the_advertised_surface` now asserts the whole core
  schema is at most 1,600 tokens.
- The acceptance suite still passes, including missing-evidence rejection,
  hidden-failure `hacked`, high-residual-risk `conditional`, self-verification
  rejection and omitted artifact digest.
- Gates: lib 1424/0/4, bin suite exit 0, fmt, strict clippy, eval 8/8.

## Limits

- Property descriptions are gone; the model relies on the prompt delta and
  field names. The live run succeeded, but an unfamiliar model may need one
  retry to learn nested shapes.
- The JSON-schema `additionalProperties` hint was removed; serde
  `deny_unknown_fields` still rejects unknown input.
- Core schema is 1,543 tokens, not exactly 1,500. The remaining cost is
  structural (required lists, types, enums), not prose.
- CLI `run` still advertises the full surface, so its schema includes the
  other tools.
