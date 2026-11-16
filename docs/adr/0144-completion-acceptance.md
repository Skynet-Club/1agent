# ADR 0144: Completion Acceptance Contract and Receipt

- Status: accepted for the P0 contract/tool and P1 runtime presence gate; P2 verification depth is pending
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
4. Add the read-only `acceptance_gate` tool. It takes one flat request
   (contract fields plus executor, verifier, results and artifact checks),
   copies criteria/artifacts into `VerificationInput`, returns the receipt JSON
   and never runs the task or a checker. The flat shape replaced an earlier
   nested `{contract, verification}` shape that real models could not fill
   reliably.
5. Add the system-prompt rule: an `accepted` receipt is required before
   `completed`; a tool run, file existence or HTTP 200 is not acceptance. Call
   the gate once and answer from the receipt; do not gate the gate.
6. Add the P1 runtime `CompletionGate`. A successful write-class tool marks the
   run as side-effecting; an `accepted` `acceptance_gate` receipt clears the
   requirement; any later write invalidates it, and a gate in the same model
   turn as a write does not count because it cannot have seen that output. A
   `completed` answer that still lacks acceptance is downgraded to
   `conditional`, gets a high-impact unknown and a flip condition, and journals
   `CompletionAcceptanceMissing`.

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
- The P1 gate proves that an accepted receipt exists after the last write. It
  does not prove that the verifier was independent or that the receipt covers
  every artifact; the model can still fabricate the verifier identity and
  evidence. P2 must run a real verifier and bind receipts to artifact hashes.
- TUI badge and receipt-listing CLI remain P1 polish; the status, conclusion and
  journal already expose the downgrade.

## Evidence

```bash
cargo test --lib acceptance
cargo test --lib tools::acceptance
cargo test --lib system_prompt_requires_capability_search_before_building
cargo test --lib completed_after_write
cargo test --lib accepted_receipt_after_write
cargo test --lib write_after_accepted_receipt
cargo test --lib completed_without_side_effects
cargo test --test tools_e2e
```

Tests cover accepted/rejected/hacked/conditional decisions, self-verification,
human verifier prefixes, missing evidence, contract/input mismatch, the flat
tool request, the runtime downgrade, the accepted-receipt path, receipt
staleness and the no-side-effect case. They do not prove a domain checker is
correct or that the verifier is independent; those remain P2.
