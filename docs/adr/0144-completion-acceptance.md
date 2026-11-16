# ADR 0144: Completion Acceptance Contract and Receipt

- Status: accepted for the P0 contract/tool slice; runtime enforcement is P1
- Date: 2026-11-16
- Related: ADR-0143, `docs/acceptance-gate-plan.md`, `src/verification.rs`

## Context

The runtime validates answer shape and evidence, and `verification::verify`
already enforces maker–checker separation, per-criterion evidence and artifact
digests. But there was no universal closing contract/receipt: a model could
report `completed` without an independent acceptance, and domain checkers such
as unslop were being confused with the acceptance mechanism itself.

The requirement is task-agnostic: every delivery needs a contract, evidence,
an independent verifier, a decision and a receipt. Domain checkers are plugins,
not the gate.

## Decision

1. Add `acceptance::AcceptanceContract`: task id/objective, public and hidden
   criteria, expected artifacts/hashes, risk, required verifier kind and
   residual risks.
2. Add `acceptance::CompletionReceipt`: contract/evidence digests, verifier
   identity and kind, verification report, decision, residual risks and
   timestamp.
3. `acceptance::close` wraps `verification::verify` and adds:
   - contract/input criteria and artifact matching;
   - verifier-kind policy (`automated`, `independent` with `verifier:`/`agent:`,
     `human` with `human:`);
   - decision mapping: verified + high residual risk → `conditional`; hidden
     failure → `hacked`; rejected → `rejected`; otherwise `accepted`.
4. Add the read-only `acceptance_gate` tool. It returns the receipt JSON and
   never runs the task or a checker.
5. Add the system-prompt rule: an `accepted` receipt is required before
   `completed`; a tool run, file existence or HTTP 200 is not acceptance.
6. P0 does not automatically downgrade every existing completed answer. P1
   adds the runtime `CompletionGate` for artifact/write tasks and the TUI
   “已验收/未验收/有条件” status.

## Consequences

- A caller can now close any task with a machine-checkable contract and an
  independent verifier, regardless of task type.
- Domain checkers (unslop, page-audit, OCR CER, ffprobe, tests) feed the
  verification input; they are not the gate.
- The receipt is only as good as its criteria and verifier; P2 adds
  capability-specific profiles and adversarial checks.
- A high-impact residual risk can never produce `accepted`; it becomes
  `conditional`.
- The tool count becomes 26.
- The model can still skip the tool in P0; the runtime hard gate is P1, and
  this ADR must not be read as universal enforcement yet.

## Evidence

```bash
cargo test --lib acceptance
cargo test --lib tools::acceptance
cargo test --lib system_prompt_requires_capability_search_before_building
cargo test --test tools_e2e
```

Tests cover accepted/rejected/hacked/conditional decisions, self-verification,
human verifier prefixes, missing evidence, contract/input mismatch and the tool
receipt. They do not prove a domain checker is correct; that remains the
checker’s own responsibility.
