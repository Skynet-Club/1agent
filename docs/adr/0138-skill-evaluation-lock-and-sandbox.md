# ADR-0138: Skill evaluation, lockfile and script sandbox

- Status: accepted for 0.98 (partial: comparator not a mandatory gate)
- Date: 2026-11-15
- Supersedes: none
- Related: ADR-0048 (skill scan), ADR-0052 (skill bundle verify),
  `src/skill_eval.rs`, `src/skill_lock.rs`, `src/skill_sandbox.rs`,
  `docs/evidence/0.98-skills/`

## Context

0.97 had SKILL.md scanning, static marker evaluation, Ed25519 bundles and
rollback. 0.98 needs an execution eval, a confidence-aware comparison, a
reproducible active set, and a boundary for bundled scripts. A skill is
untrusted text plus optional files; a “passing” score with no held-out split or
statistical interval is not evidence.

## Decisions

1. **Validator first.** A skill directory is rejected for symlinks, path
   escapes, oversize files, instruction-override markers; scripts and external
   references are reported for human review.
2. **Execution eval hashes outputs.** `skill eval-runtime` runs a model/runner
   over held-out prompts and stores only pass/fail, output SHA-256 and length;
   raw outputs are not persisted in the report.
3. **Activation gate.** Activation requires a passing eval report or an
   explicit audited `--force`; the default is refuse.
4. **Blind comparator.** `compare_blind` assigns candidate/baseline to A/B
   with a seeded SHA-256, scores each arm without knowing labels, and reports
   Wilson 95% intervals for pass-rate lift. `skill compare` exits nonzero
   unless the conservative lower bound is positive. It is not yet a mandatory
   activation gate.
5. **Held-out split.** `split_runtime_cases` orders cases by seeded hash and
   requires a non-empty train and test side; tiny suites fail closed.
6. **Active-set lockfile.** `skill lock` writes name/version/content SHA-256
   for active skills; `skill verify-lock` requires an exact two-way match, so
   a deploy cannot silently add an unpinned active skill.
7. **Bundles stay signed.** Ed25519 signing, SHA-256 gates and immutable
   versions are unchanged.
8. **Scripts never run implicitly.** The runtime does not execute skill
   scripts during activation or normal runs. `skill run-script` is explicit
   and uses bwrap: no network, no host home, read-only `/usr` and skill dir,
   tmpfs `/tmp`, dropped capabilities, timeout and output caps. Missing bwrap
   fails closed; there is no unsandboxed fallback.

## Consequences

- A skill can pass marker eval without being generally useful; the blind
  comparator and lockfile reduce, but do not eliminate, that risk.
- The sandbox is Linux/bwrap-only; elsewhere `skill run-script` fails closed.
- A malicious skill can still instruct a terminal-capable model to do harm;
  script sandboxing is not a substitute for terminal permissions/Authority.

## Tests

- `src/skill_validate.rs`, `src/skill_eval.rs`, `src/skill_lock.rs`,
  `src/skill_sandbox.rs` unit tests.
- `tests/skill_validate_cli.rs`: validator, activation gate, blind CLI,
  lock/verify-lock, bwrap run-script and traversal refusal.
- `tests/skill_canary_cli.rs`: persistent canary and rollback.
