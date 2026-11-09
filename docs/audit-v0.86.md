# Audit v0.86.0

Date: 2026-11-02

## Outcome

The repository is clean at the v0.86.0 release-evidence commit. Releases v0.73.0 through
v0.86.0 are tagged, documented and backed by reproducible Linux/Windows packages.

## Current verification

| Check | Result |
|---|---|
| `cargo fmt --check` | passed |
| Clippy with `-D warnings` | passed |
| Unit tests | 123 passed |
| Release-profile tests | 123 unit + all E2E passed |
| Offline integration / CLI E2E | passed |
| Core contract evaluation | 6/6 passed |
| Core hard-gate failures | 0 |
| v0.86 clean-source package gate | passed |
| Linux archive reproducibility | passed |
| Windows archive reproducibility | passed |
| Release manifest verification | passed |
| Linux provenance verification | passed |

## Historical release audit

The current v0.86 verifier checked every retained release manifest from v0.71.0 through
v0.86.0:

```text
0.71.0 verified ... 0.86.0 verified
```

This proves the retained manifests and referenced package material still reconcile with the
current release verifier. It does not retroactively prove real-model quality or Windows runtime
behavior.

## Requirement audit

| Requirement | Evidence | Remaining gap |
|---|---|---|
| Provider-neutral agent core | canonical contracts, deterministic provider, OpenAI-compatible adapter, routing tests | live provider quality |
| Result-first and no fake certainty | answer v2, evidence provenance, unknown/flip gates, decision completeness | calibrated semantic quality |
| Facts/assumptions/constraints/unknowns/conflicts/flips | typed domain contracts and semantic audit | reviewer calibration on real tasks |
| Provider routing | capability filters, multi-objective ranking, budgets, fallback journaling | cross-provider quality data |
| Gated self-evolution | candidate/review/approval/held-out verification/digest/activation/supersession/expiry | measured positive transfer on real held-out tasks |
| Durable execution | JSONL journal, checkpoint, resume, route-bound recovery | multi-writer store and long-duration live tests |
| Host integration | stdio protocol, concurrency, cancellation, summaries | authenticated network transport not implemented |
| Cross-platform | platform-neutral core and Linux/Windows archives | Windows execution unverified |
| Report trust | policy/integrity/digest/HMAC/trust requirements/negative paths | asymmetric signatures and external anchor validation |
| Documentation flywheel | status, architecture, ADR, audit and Obsidian mirror | continue per future evidence |
| Terminal boundary | standalone project; no terminal modifications | none for current scope |

## Open evidence gaps

- No configured live DeepSeek, Kimi, Qwen or Ollama run.
- No real held-out model-quality or calibration report.
- No real Windows-host execution.
- No second-host hermetic reconstruction.
- No macOS runtime verification.
- No asymmetric report identity or public trust anchor.

## Decision

Hold further autonomy expansion. Execute
[`next-phase-evidence.md`](next-phase-evidence.md) before choosing an asymmetric report-signature
implementation or increasing self-evolution scope.
