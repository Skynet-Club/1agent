# ADR 0013: Explicit Artifact Verification and Master Views

Status: accepted

Date: 2026-11-03

## Context

An Agent process can exit zero without producing the expected delivery. A
production Master must therefore inspect actual files, require independent
criterion evidence, and expose the resulting claims as a replayable production
record rather than treating execution success as acceptance.

## Decision

Add Master-only verification commands:

- `/artifacts [task-id|all]` reloads `MasterState`, re-inspects every expected
  Artifact under its Work Item workspace, and reports presence, regular-file
  status, size, actual SHA-256, pinned SHA-256 and a deterministic verdict.
- `/verify <task-id> <verification.json>` accepts the existing explicit
  `VerificationSubmission` shape. It computes Artifact checks itself, requires
  the verifier identity to differ from the assigned execution Agent, calls
  `MasterState::verify`, and journals `Verified` or `Rejected`.

Verification derives acceptance from four independent facts:

1. the receipt exists and the execution completed with exit code zero;
2. every exact acceptance criterion has one passing result with evidence;
3. every expected Artifact exists as a regular file below its workspace;
4. any pinned SHA-256 equals the actual file digest.

Master views reuse the same durable state:

- `/evidence` renders acceptance criteria as claims and binds them to receipts,
  checked Artifacts and independent verifier evidence;
- `/audit` replays the Master journal and additionally checks leases, execution
  targets, receipt generations and current Artifact drift; in Master mode,
  `/audit all` also includes bounded run-journal audits;
- `/report` renders production counts and separate planned, executed, verified
  and decided sections.

Master `/evidence`, `/audit` and `/report` use the Master record by default after
dispatch. `/evidence run`, `/report run` and `/audit run` retain the Core run
views.

## Consequences

- Exit code zero cannot satisfy verification without the expected delivery.
- Missing criteria, missing Artifacts, digest drift, test failures and
  self-verification fail closed.
- The verifier sees the current filesystem state rather than trusting a prior
  receipt.
- Master audit can expose a missing or changed artifact after an earlier
  verification.
- Verification input remains explicit and bounded; no model is asked to certify
  its own output.

## Verification

- `/artifacts` prints the expected and actual Artifact state;
- `/verify` updates and journals Verified or Rejected state;
- missing Artifact false-success and SHA-256 drift are rejected;
- a missing acceptance criterion is rejected;
- the assigned execution Agent cannot verify its own task;
- Master evidence, report and audit are deterministic journal/state views;
- Master journal content tampering is detected;
- existing run-journal audit tests detect unpaired tool calls;
- a CLI TUI E2E runs all five phase-five commands and ends with a Verified task.
