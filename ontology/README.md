# 1AO — 1Agent Ontology Draft

Status: **draft, not wired into the runtime**

This directory contains the machine-readable companion to
[`docs/super-agent-ontology.md`](../docs/super-agent-ontology.md).

| File | Purpose |
|---|---|
| `1agent-context.jsonld` | JSON-LD 1.1 context (`one:`, PROV-O mapping, common properties) |
| `1agent-core.ttl` | Core classes/properties; `Run`/`Agent`/`Artifact` aligned to PROV-O |
| `1agent-shapes.ttl` | SHACL validation shapes for exported data |
| `bindings.example.json` | Illustrative type/property → journal/session/registry bindings |
| `examples/run.jsonld` | Example run + task + tool call + answer + claim + evidence + artifact |

## What is stable

- Instance IRI rules and deterministic export are design targets, not yet code.
- `journal`, `session.json` and `subagent_registry.jsonl` remain the source of
  truth. The ontology is a read-only projection.
- The namespace `https://1agent.dev/ontology/0.98#` is provisional; instances use
  `urn:1agent:` until the owner confirms the domain.

## Validation

- JSON files: `jq empty ontology/1agent-context.jsonld ontology/examples/run.jsonld`
- Turtle: use `rdflib`, Apache Jena `riot`, or `pyshacl` when available.
- No runtime dependency and no model-prompt change is introduced by this
  directory.

## Not in this draft

- A full OWL 2 DL axiomatization.
- A triple store, reasoner, or SPARQL endpoint.
- Raw prompts, model chain-of-thought, secrets, or file contents.
- A replacement for the existing JSON Schema or journal audit.
