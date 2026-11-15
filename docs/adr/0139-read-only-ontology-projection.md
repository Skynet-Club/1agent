# ADR-0139: Read-only ontology projection (1AO)

- Status: accepted for 0.98.2
- Date: 2026-11-16
- Supersedes: none
- Related: `docs/super-agent-ontology.md`,
  `docs/0.98.2-ontology-contracts.md`, `src/ontology/`, `ontology/`

## Context

1Agent already records runs in JSONL journals, sessions in `.sessions`, and
sub-agent lineage in a registry. The records are auditable, but external audit,
cross-session questions and Super-Agent delegation need stable identities,
typed entities and explicit claim/evidence lineage. A full OWL reasoner or a
second graph store would add operational cost and split the source of truth.

## Decisions

1. **The journal remains the source of truth.** The ontology is a read-only
   projection. Export failure does not change a run, and no runtime decision
   depends on the projection.
2. **Typed registry plus bindings.** `ontology/types.json` declares the core
   entity types; `ontology/bindings.json` maps them to journal/session fields.
   Both are versioned and hashed. The exporter embeds `ontologyVersion`,
   `bindingVersion` and `registrySha256`.
3. **Deterministic JSON-LD.** Export is self-contained JSON-LD with an embedded
   context, `@graph` sorted by `@id`, and no export timestamp. The SHA-256 is
   computed over the exact serialized bytes, so the same input produces the same
   bytes and hash.
4. **Stable IRIs, attributes not identity.** Session, run, task, tool-call,
   observation, answer, claim, evidence, artifact, goal and commitment IRIs
   follow the P0 contract. PID, title and path are attributes, never identity.
   Artifact identity is content SHA-256.
5. **Allowlist and redaction.** The exporter writes identifiers, types, times,
   statuses, hashes, references and bounded summaries. It does not export raw
   prompts, tool content, page text, credentials or file bodies.
6. **Validation is internal and fail-visible.** Rust validation checks required
   fields, enums, duplicate/dangling IDs, artifact SHA shape and core
   relations. An invalid export exits non-zero; it is never silently repaired.
   SHACL/Jena is a release-time cross-check, not a runtime dependency.
7. **Lineage is additive.** Sub-agent parent/spawn fields, session fork IDs,
   read-file SHA metadata and optional policy/approval metadata use
   `serde(default)` so v1/v2 sessions and old journals remain readable.
8. **No graph database, no OWL reasoner, no A2A/OTel in 0.98.2.** Those remain
   evaluated options for later versions, not hidden scope.

## Consequences

- The ontology cannot answer facts that were never journaled; missing action,
  policy or approval data stays `Unknown` rather than guessed.
- Registry and binding drift are a release risk. The version/hash pair and
  schema tests make drift visible, but do not eliminate it.
- The export is a deterministic audit artifact, not a complete semantic model
  of the agent. Claim status is not a calibrated probability.
- Entity merges/aliases and full bitemporality are deferred; P0 only records
  what the existing journal can support.

## Tests

- `src/ontology/types.rs` and `src/ontology/ids.rs` unit tests.
- `src/ontology/export.rs` / `validate.rs` deterministic, compatibility and
  redaction tests.
- CLI smoke tests for `1agent ontology types|bindings|export|validate`.
- Independent QA: golden export, malformed input, v1/v2 session compatibility,
  no-secret/no-prompt checks, and at least the contracted competency questions.
