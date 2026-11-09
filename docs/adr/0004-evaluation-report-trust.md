# ADR 0004: Evaluation Report Trust Boundary

Status: accepted

Date: 2026-11-02

## Context

Evaluation reports are consumed by future hosts and may drive promotion decisions. A report
needs stable decoding, internal consistency checks, mutation detection and an optional
authentication proof. Conflating those properties would make an ordinary content hash look like
a trusted attestation.

## Decision

- Reports carry a decision-completeness policy version and floor-source metadata.
- Reports use a canonical SHA-256 digest over normalized content with the digest field cleared.
- Summaries have an independent schema version and exclude case content.
- Inspection always validates policy, arithmetic, aggregate and digest invariants.
- Hosts may additionally require content binding.
- Authenticated inspection reuses `ApprovalEnvelope` with artifact kind `evaluation_report`.
- Authentication is HMAC-SHA256 over exact report bytes and is constrained by the approval key
  registry.
- Summary verification metadata distinguishes integrity, content binding, signature presence and
  authenticity.
- Unauthenticated reports remain available in informational mode but never satisfy
  `--require-authenticity`.

## Options considered

### Trust every JSON report

Rejected: malformed or contradictory reports could influence downstream decisions.

### Treat SHA-256 as a signature

Rejected: a party able to rewrite the payload can recompute the digest.

### Invent a separate report-signing protocol

Rejected for V0: the repository already has approval envelopes, key lifecycle controls and
registry verification. Reuse reduces protocol and custody surface, with symmetric-key limits
documented explicitly.

### Require signatures for every inspection

Rejected: diagnostic inspection of unsigned reports remains useful. Hosts opt into stricter
requirements.

## Consequences

- Report consumers must distinguish integrity from authenticity.
- Existing unbound reports and summaries remain readable for compatibility.
- The current HMAC design is suitable for controlled shared-key deployments, not public
  third-party attestations.
- A future asymmetric signature can use the same trust-state contract while replacing the proof
  mechanism.
- Report approval does not replace release approval or trust-anchor chain verification.
