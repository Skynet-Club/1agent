# Audit v0.87.0

Date: 2026-11-02

## Release state

- version: `0.87.0`
- feature commit: `b9dd9dd18c6901d07876c6a7554b57ba723b7b28`
- tag: `v0.87.0`
- dirty worktree before audit-doc commit: false
- frozen terminal: not modified

## Verification snapshot

| Check | Result |
|---|---|
| Formatting | passed |
| Clippy with `-D warnings` | passed |
| Unit tests | 123 passed |
| Release-profile tests | 123 unit + all E2E passed |
| Offline integration / CLI E2E | passed |
| Core contract evaluation | 6/6 passed |
| Core hard-gate failures | 0 |
| Clean-source package gate | passed |
| Linux archive reproducibility | passed |
| Windows archive reproducibility | passed |
| Release manifest verification | passed |
| Linux provenance verification | passed |
| Relative Markdown links | passed |

## Historical release chain

The v0.87 verifier accepted every retained manifest from v0.71.0 through v0.87.0.

## Requirement status

Implemented and offline verified:

- provider-neutral task/model/tool/answer contracts;
- structured result-first answers;
- evidence, unknown, conflict and flip-condition validation;
- decision-completeness scorecards and configurable floors;
- capability-based routing with bounded fallback;
- journal, audit, checkpoint, resume and route-policy binding;
- gated L2 experience evolution;
- report policy/integrity/digest/HMAC/trust-state inspection;
- case-content-free summaries and human-readable trust labels;
- Linux/Windows deterministic package builds.

Not verified:

- live DeepSeek, Kimi, Qwen or Ollama quality;
- real held-out model quality, calibration or positive experience transfer;
- Windows execution on a real Windows host;
- cross-machine hermetic reconstruction;
- macOS runtime;
- asymmetric identity and external report trust anchors.

## Boundary decision

Continue only with the environment-evidence work in
[`next-phase-evidence.md`](next-phase-evidence.md). Do not increase autonomy or replace the
current trust contracts based on fixture-only results.
