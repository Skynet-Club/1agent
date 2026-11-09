# Handoff: v0.90.0

Date: 2026-11-02

## Current release

- version: `0.90.0`
- feature commit: `bf393a15c4c8bd4f234282bc21ab3d35e8d40b89`
- tag: `v0.90.0`
- repository status at handoff: clean

## Completed in this work cycle

Evaluation and report trust:

- decision-completeness per-case and suite scorecards;
- bounded completeness floors;
- policy version and floor-source metadata;
- external report inspection with policy validation;
- aggregate integrity validation;
- case-free machine summaries;
- summary schema versioning;
- canonical report content digest;
- explicit integrity/content/signature/authenticity states;
- host-selectable content-binding and authenticity requirements;
- HMAC-authenticated report inspection and negative-path tests;
- human-readable floor and trust states.

Security hardening:

- report proof arguments validated before file access;
- overflow-safe report count checks;
- non-loopback Provider credentials require HTTPS;
- empty and URL-embedded credentials rejected;
- readiness preflight uses the same credential transport rules as provider construction.

## Verification evidence

- unit tests: 127 passed;
- release-profile tests: 127 unit tests and all E2E passed;
- offline integration and CLI E2E: passed;
- core contract evaluation: 6/6, zero hard-gate failures;
- Clippy with `-D warnings`: passed;
- clean-source package gate: passed;
- Linux and Windows archive reproducibility: passed;
- current and historical manifests v0.71–v0.90: verified;
- relative Markdown links and Obsidian section structure: passed.

The v0.90 release hashes are recorded in
[`status-v0.90.md`](status-v0.90.md).

## Environment constraints encountered

An earlier build exhausted the root filesystem. Cargo incremental artifacts were the effective
recovery target. The final release was completed after sufficient space became available.

Do not continue release work if `bwrap` or Cargo reports `No space left on device`.

## Not verified

- live DeepSeek, Kimi, Qwen or Ollama quality;
- production latency and cost of those providers;
- reviewed held-out model quality and calibration;
- positive experience transfer on real tasks;
- Windows runtime on a real Windows host;
- cross-host hermetic reconstruction;
- macOS runtime;
- asymmetric report signatures and public trust anchors.

These remain evidence gaps, not implementation placeholders.

## Next action

Execute [`next-phase-evidence.md`](next-phase-evidence.md) in order:

1. approve the evaluation corpus and reviewer process;
2. configure one provider without exposing credentials;
3. run adapter smoke and preserve raw reports;
4. run the provider pilot;
5. repeat across providers and Ollama;
6. execute Windows and cross-host gates;
7. decide whether the V0 architecture needs revision.

Do not increase autonomy or claim production model quality from deterministic fixture results.
