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

- 1h M1 real-task gate: needs a provider key with tool calling;
- 8h model-quality soak: needs a provider;
- 100-question suite rerun: needs a provider with tool calling;
- W1/W2/W3 real workflows: need a provider and owner systems/accounts;
- adaptive red-team/OWASP report and independent review;
- clean commit + tag, package, SBOM/attestations/provenance;
- owner approval to publish.

The tree is intentionally uncommitted. Do not package or publish until the
owner approves a commit/tag and the remaining gates are green or explicitly
accepted in release notes.
