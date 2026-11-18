# ADR 0150: Decision and action ontology layer

- Status: proposed
- Date: 2026-11-18
- Related: 1AO (`docs/super-agent-ontology.md`), 1.1 standard harness plan §11

## Context

Palantir's 2026 Ontology material describes an operational layer combining
Data, Logic, Action and Security. The valuable parts for 1Agent are not RDF
graphs but:

- decisions as first-class objects with options, criteria, evidence, outcome
  and lineage;
- typed `ActionType` transactions with parameters, preconditions, submission
  criteria, side effects, approval and postconditions;
- object/property/cell-level security tied to tool access;
- staged scenarios/proposals before committing;
- reads exposed as query tools, writes as one typed tool per action.

1Agent already has journal/evidence, capability compiler, ActionCard,
Authority/Policy, approvals and task graph, but they are not joined into a
decision/action model. The current 1AO is a read-only projection.

## Decision

1. Keep `journal` as the only source of truth; the ontology remains a
   deterministic, permission-aware projection.
2. P0 adds read-only `Decision`, `Option`, `Criterion`, `ActionType`,
   `ActionInvocation`, `PolicyDecision`, `Approval`, `Outcome` and `Scenario`
   types derived from existing data. No prompt/journal/answer schema change.
3. P1 introduces a typed `ActionType` registry. Each write action generates a
   `ToolSpec`/`CapabilityManifest`; undeclared actions must not execute.
4. P2 adds scenario/proposal staging and atomic merge; no production mutation
   before approval.
5. P3 adds object/action-level policy and an Ontology MCP surface (reads as
   query tools, writes as action tools).
6. Do not add an OWL reasoner, graph database migration or full Palantir clone.

## Consequences

- 1Agent can report decisions and action lineage, not only tool traces.
- Typed actions reduce the attack surface of generic write tools and make
  approvals auditable.
- New action types need schema review, versioning and tests; this adds process
  cost and a risk of action/tool proliferation.
- Policy scopes can grow complex; start with object-type + action scope.
- 1ASH can include decision/action tasks without using the ontology export as an
  oracle, avoiding circular self-evaluation.
