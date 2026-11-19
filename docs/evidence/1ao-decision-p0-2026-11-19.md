# 1AO decision/action P0 — first slice (2026-11-19)

Scope: read-only ontology export. No prompt, journal event, answer schema or
runtime behavior changed.

## Implemented

- `Decision` type and deterministic IRI
  `urn:1agent:decision:<run>:main`.
  - hashed objective (`questionSha256`, `questionChars`);
  - status from `AnswerProduced.answer.status`;
  - links to Run, Answer, Claims, Evidence and the Outcome record.
- `Outcome` type and IRI `urn:1agent:outcome:<run>`.
  - status from `RunStopped.reason` (fallback: answer status);
  - link to the Decision and generated artifacts.
- `Run.hasDecision` reference.
- `Option` nodes from `ReframeProposed` options, with selected/rejected/proposed
  status and hashed option text; `Decision.hasOption` and `chosenOption`.
- `Criterion` nodes from the reframe stop rule (verifiable, deliverable,
  better-substitute), with source asserted/derived; `Decision.hasCriterion`.
- Type-registry entries for `Decision`, `Outcome`, `Option`, `Criterion`; data
  bindings for question digest, statuses, option index and stop-rule criteria.
- Validator knows all four types and their status/field rules; unknown
  references and malformed statuses still fail closed.

## Action Registry first slice

- `ontology/actions.json` defines the code-change/release vertical:
  `code.propose_change`, `code.run_checks`, `code.approve_change`,
  `code.prepare_release`.
- Each action carries parameters JSON Schema, preconditions, risk,
  reversibility, approval policy, side effects, postconditions and idempotency.
- `1agent ontology actions` lists the registry; `--tools` prints generated
  ToolSpec JSON and `--manifests` prints generated CapabilityManifest JSON.
- This is read-only registry data; runtime typed-tool registration and gating
  are not wired yet for the remaining actions.

## Typed tool first slice

- `code.propose_change` is registered as `code_propose_change` in the default
  tool registry; its ToolSpec comes from the ActionType registry.
- It writes a 0600 proposal artifact under `.1a/proposals/<id>.json`, never
  modifies the target files, and goes through the normal ExecutionGate.
- The observation carries `action_type`, proposal id/path/SHA-256, risk and
  reversibility; ontology export links `ActionInvocation`, `Decision.hasAction`
  and a `Proposal` node (`proposed`, SHA-256, action/decision lineage).
- The other three ActionTypes remain registry-only; approval/release execution
  is not claimed.
- Human decision path: `1agent action approve-change --proposal <path>
  --decision approve|reject --actor <name> --yes` records a 0600 receipt under
  `.1a/approvals/<proposal_id>.json`; identical decisions are idempotent.
  This is a CLI/library operation, not a model tool, so an agent cannot
  self-approve. It records the decision; it does not merge or release.

## Checks and local release

- `code.run_checks` validates each proposal path stays inside the workspace,
  records SHA-256/bytes for each file, and writes a 0600 check report under
  `.1a/checks/<proposal_id>.json`; missing/oversized paths fail the report.
- `code.prepare_release` requires an approved receipt whose proposal SHA-256
  matches and a passed check report for the same proposal; it writes a 0600
  local release manifest and SHA256SUMS under `.1a/releases/<proposal_id>/`.
- It does not build/package binaries and does not publish externally; that is
  a separate future action.
- `code.approve_change` remains a human CLI/library operation, not a model tool.

## Verification

| Check | Result |
|---|---|
| `./scripts/verify.sh` | **pass**; 1666 total, 0 failed, 13 ignored |
| ontology/unit tests | 34 ontology + 7 action tests |
| ontology CLI tests | 29 passed |
| real journal smoke | Decision + Outcome nodes exported and validated |
| claims | 266 (242/16/8) |

Raw: `verify-1ao-decision-p0.log`, `verify-1ao-option-criterion.log`, `verify-1ao-action-registry.log`, `verify-1ao-action-tools.log`, `verify-1ao-typed-tool.log`, `verify-1ao-proposal.log`, `verify-1ao-proposal-approval.log`, `verify-1ao-release-tools.log`.

## Not in this slice

- `Scenario` merge/atomic staging (Proposal is read-only representation).
- Real build/package execution; `prepare_release` writes a manifest only.
- Object/action-level policy and Ontology MCP.

## Next

1. Finish decision/action CQ golden tests.
2. Wire `code.approve_change` to a human approval path, then `code.run_checks`
   and `code.prepare_release`; add `Scenario` staging (P2) after that.
