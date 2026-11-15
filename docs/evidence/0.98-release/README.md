# 0.98 Release Gate Evidence (Working)

Date: 2026-11-15  
Gate: G7

## Current local verification

```bash
./scripts/verify.sh
```

Result: fmt, clippy `-D warnings`, full test suite, core eval 8/8 (hard-gate
failures 0, rubric 6/6), claims 251 (227 verified / 16 partial / 8 no evidence).
Raw tail: `verify.txt`.

## Not yet satisfied

- 1h M1 real-task gate: **rerun3 239/240 = 99.58%**, completed replay
  100%, 0 crashes (`../m1-1h-2026-11-15-rerun3/`); the binary predates the
  TUI interaction changes, which `benchmark-run` does not use;
- 8h soak: needs an owner-approved window; deterministic recovery soak can run
  without a provider, model-quality soak needs a model budget;
- 100-question suite rerun: provider is configured (`deepseek-flash`); needs
  the run;
- W1 real workflow is complete (`docs/evidence/0.98-w1/`); W2/W3 need owner
  systems/accounts;
- adaptive red-team/OWASP report and independent review;
- clean commit + tag, package, SBOM/attestations/provenance;
- owner approval to publish.

The tree is intentionally uncommitted. Do not package or publish until the
owner approves a commit/tag and the remaining gates are green or explicitly
accepted in release notes.
