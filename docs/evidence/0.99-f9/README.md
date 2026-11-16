# 0.99 F9 — ontology binding coverage evidence

Date: 2026-11-16
Scope: close the 0.98.2 F9 gap by declaring owner/mapping version on every
binding, covering every core type and every required property, and validating
the registry.
Result: **complete and validated; the binding mapping version is now `2`.**

All fixtures are synthetic. No production journals, prompts, credentials or
secrets are included.

## 1. Tested tree

| Field | Value |
|---|---|
| Repository | `/home/skynet/1-agent` |
| Branch | `0.99-dev` |
| Slice tag | `0.99-f9-binding-coverage` |
| Changed files | `ontology/bindings.json`, `src/ontology/mod.rs`, `src/ontology/types.rs`, `src/ontology/validate.rs`, `src/main.rs`, `tests/ontology_cli.rs`, `docs/architecture.md`, `docs/0.99-handoff.md` |

## 2. Commands and results

```bash
cargo test --offline --lib
cargo test --offline --test ontology_cli
./scripts/verify.sh
```

| Command | Result |
|---|---|
| `cargo test --offline --lib` | **1115 passed, 0 failed, 4 ignored** |
| `cargo test --offline --test ontology_cli` | **26 passed, 0 failed** |
| `./scripts/verify.sh` | **passed** (fmt, clippy all-targets `-D warnings`, all tests, core-v3 8/8) |

What changed:

- Every binding has `owner` and `mappingVersion: "2"`.
- Mappings added for Task, ToolInvocation.name, Observation, Answer,
  Claim.claimText, Evidence.sourceRef/supportsClaim, SquadMember.status,
  Goal, Commitment, ActionType risk/reversible/hasAccessPolicy,
  ActionInvocation.status, PolicyDecision, Approval and DataBinding
  boundTo/sourcePath/mappingVersion.
- `binding_coverage(types, bindings)` checks unknown types, owner mismatch,
  unsupported mapping version, unknown source prefixes, and
  `required`+`required_after` conflicts; coverage only counts valid bindings.
- `1agent ontology bindings --coverage [--json]` prints the report and exits
  non-zero when incomplete.
- `BINDING_VERSION` and `bindings.json#mapping_version` are `2`; export
  assertions updated.

Tests:

- `ontology::types::tests::builtin_bindings_parse_and_reference_compact_ids`
  now asserts complete coverage, no unbound types/required properties.
- `ontology::types::tests::binding_coverage_reports_unknown_types_owners_sources_and_versions`
  — all five error classes.
- `tests/ontology_cli.rs::binding_coverage_is_complete_and_validated` — CLI
  report `ok`, all types bound, every required property has a binding.

## 3. Acceptance mapping

| F9 criterion | Evidence |
|---|---|
| sourcePath existence | Source must use a known prefix set; bad source is an error |
| mappingVersion | Required and must equal the current version |
| owner | Required and must match the type owner |
| required | Every type's required properties have a valid binding; `required`+`required_after` conflict is an error |
| Coverage | All 17 core types bound; CLI report exits non-zero when incomplete |

## 4. Explicit non-claims

- Source paths are validated against a known prefix set, not against a
  machine-readable journal schema; a deeper field-existence check remains
  future work.
- The mapping version bump is a registry contract change; exports now carry
  `bindingVersion: "2"`.
- CQ11/CQ15, the independent CQ re-evaluation and QG1/QG2/QG3 remain open.
