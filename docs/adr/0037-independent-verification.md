# ADR 0037: Independent Verification and Hack-Verifiable Acceptance

- Status: accepted
- Date: 2026-11-11

## Context

A worker's "completed" report is not evidence. Agents can satisfy the public
checks by shortcutting the task, and an executor that verifies itself can hide
the shortcut. Evaluation reports already carry arithmetic and content-binding
checks, but the runtime lacked a small, reusable acceptance contract that makes
the distinction explicit.

The relevant failure mode is not only a wrong answer. It is a **claimed
resolution**: the public test passes, the hidden test fails, and the report is
counted as success. A useful acceptance gate must name that state.

## Decision

1. Add `src/verification.rs`.
2. `verify(input)` fails closed on malformed input:
   - executor and verifier identities must be non-empty and different;
   - there must be at least one acceptance criterion;
   - every acceptance and hidden criterion needs exactly one result;
   - a passing result needs non-empty evidence;
   - every expected artifact needs exactly one check.
3. A valid submission is classified as:
   - `Verified`: public and hidden criteria pass, artifacts pass and digests
     match;
   - `Rejected`: any public criterion or artifact check fails;
   - `Hacked`: all public checks pass but a hidden check fails.
4. `summarize(reports)` reports `resolved`, `verified`, `hacked`, `rejected`
   and `hacked_resolved_bps = hacked / resolved`. A high public pass rate
   cannot hide reward hacking.
5. Hidden criteria are never shown to the executor. The verifier is a distinct
   identity; self-verification is an error, not a rejected report.
6. This contract is deterministic and offline. It does not claim that a real
   model reasons well; it proves the acceptance plumbing distinguishes clean
   from hacked resolutions.

## Consequences

- Runtime verification can use one contract instead of ad-hoc checks.
- Eval reports can add `hacked_resolved_bps` without changing the answer
  schema.
- A verifier that is accidentally the executor fails closed.
- Hidden tests must be maintained separately from public tests, or the
  `Hacked` class disappears.

## Evidence

```bash
cargo test verification --lib
cargo test --test hack_verifiable
```

The fixture covers clean, shortcut, artifact tamper, public failure,
self-verification and missing evidence. It is a contract fixture, not a
model-quality claim.
