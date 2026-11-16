# Completion Acceptance Gate — Development Plan

Status: P0 implemented; P1–P3 planned
Date: 2026-11-16
Related: [ADR-0144](adr/0144-completion-acceptance.md), `src/verification.rs`, `src/verification_store.rs`, [capability scouting](capability-scouting-plan.md)

## 0. One sentence

A task is complete only when an acceptance contract has been checked by evidence and an independent verifier, and the result is recorded as a receipt. The model saying “done” is a claim, not a completion.

## 1. Problem

The current runtime already validates answer shape, claims, evidence, goal closure and some independent verification, but there is no universal closing gate:

- `AnswerStatus::Completed` can be produced from the model’s own conclusion.
- Design review (unslop/web-ui-audit) is only one domain checker, not the acceptance mechanism.
- A file exists / HTTP 200 can be mistaken for delivery.
- The maker can be the only reviewer.
- A “pass” can carry unresolved high-impact unknowns or missing artifact hashes.

The user’s requirement is general: **any task**, not just web pages, must have a professional closing mechanism.

## 2. Principles

1. **Contract before work.** Acceptance criteria, required evidence, verifier role and stop conditions are defined before execution.
2. **Evidence, not assertion.** The maker’s “done” is a claim; tests, hashes, screenshots, metrics, approvals and journal records are evidence.
3. **Maker–checker separation.** The executor cannot be the sole verifier for medium/high risk. Use an independent verifier agent/model or a human.
4. **Four dimensions.** Functional correctness, quality/non-functional, safety/authorization, and user outcome.
5. **Risk-scaled depth.** L0 automated checks; L1 domain checkers; L2 independent review; L3 human sign-off. The risk decides the required level.
6. **Fail closed.** Missing evidence, unknown = not passed. High-impact residual risk means conditional at best.
7. **Receipt, not sentiment.** Completion returns a bounded, auditable `CompletionReceipt` tied to artifact hashes/version.
8. **Domain checkers are plugins.** unslop, page-audit, OCR CER, ffprobe, tests, security scans are L1 checkers; they feed the universal gate.
9. **The gate itself is auditable.** Track false accept, false reject, rework and cost per verified outcome.
10. **Know when to stop.** Acceptance has thresholds; it is not infinite polishing.

## 3. Architecture

```text
Task
  → AcceptanceContract (criteria, hidden criteria, artifacts, risk, verifier)
  → execution + evidence
  → VerificationInput (executor, independent verifier, criterion results, artifacts)
  → verify() (existing module: self-verification, missing evidence, hidden checks)
  → CompletionReceipt (accepted / conditional / rejected / hacked)
  → CompletionGate (completed requires an accepted receipt)
  → journal / ontology / Bitable
```

### 3.1 AcceptanceContract

- task id and objective;
- public acceptance criteria;
- hidden/adversarial criteria;
- expected artifacts and optional SHA-256;
- risk level;
- required verifier kind (`automated`, `independent`, `human`);
- residual risks;
- stop conditions.

### 3.2 CompletionReceipt

- contract digest and evidence digest;
- verifier id and kind;
- verification report (public/hidden pass, failures);
- decision and residual risks;
- created timestamp.

### 3.3 Gate phases

- **P0 (this slice):** contract/receipt model, `acceptance_gate` tool, independent verifier check, prompt policy, tests. No default runtime downgrade yet.
- **P1:** runtime `CompletionGate`: completed answers for artifact/write tasks require an accepted receipt; TUI shows “已验收/未验收/有条件”; CLI to inspect receipts.
- **P2:** capability-specific verification profiles; verifier subagent; hidden/adversarial checks; target-environment checks (390/768/1440, OCR CER, ffprobe, etc.).
- **P3:** ontology/Bitable receipts; gate metrics (false accept/rework); human sign-off workflow; periodic revalidation tied to artifact hashes.

## 4. P0 acceptance criteria

- `acceptance_gate` is listed in `1agent tools`.
- A valid contract + independent verification returns `accepted`.
- Self-verification, missing evidence, missing artifact, hidden-check failure and high residual risk produce the correct error/decision.
- The system prompt requires an accepted receipt before delivery and forbids treating a tool run as acceptance.
- Full `./scripts/verify.sh` passes.
- The tool is read-only; it does not execute the task or install checkers.

## 5. P0 non-goals

- No automatic downgrade of every existing run; that is P1.
- No universal “one checklist” — domain checkers remain pluggable.
- No claim that structural checks prove user value; they prove the contract was met.
- No automatic human review.
- No change to Authority/Policy.

## 6. Risks

- **Checklist theater:** criteria can be gamed; hidden/adversarial checks and independent verifiers reduce it, gate metrics expose it.
- **Overhead:** not every low-risk task needs a human; risk-scaled levels keep it proportional.
- **Stale receipts:** bind receipts to artifact hashes/versions; changes invalidate.
- **False confidence:** a passed contract only proves the stated criteria, not perfection.
- **Model bypass:** P0 is tool + prompt; P1 must enforce in the runtime.

## 7. Verification

```bash
cargo test --lib acceptance
cargo test --lib system_prompt_requires_acceptance_gate
cargo test --test tools_e2e
./scripts/verify.sh
```

The tests prove the acceptance contract, independent verifier, evidence and artifact checks, decisions and tool contract. They do not prove that a future domain checker is correct or that the model always calls the gate; P1 adds runtime enforcement.
