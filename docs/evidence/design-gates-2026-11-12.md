# Design Gates (SA97-05 / SA97-06) — Evidence (2026-11-12)

Status: **documents accepted for migration planning; local review only**
Scope: architecture boundary RFC and P0/P1 threat model.

## Artifacts

- `docs/rfc/0001-module-boundaries.md`
- `docs/threat-model.md`
- `docs/documentation-map.md` entries

## What this proves

- The session/harness/sandbox contracts are written down with their intended
  surfaces and dependency rules.
- `main.rs` (~20,273 lines) and `tui.rs` (~7,932 lines) have an ordered,
  revertible split plan; no behavior change is proposed.
- The threat model lists P0/P1 threats across provider output, tools, skills,
  MCP, journals, daemon/webhooks, CUA and platform boundaries, with existing
  mitigations and residual gaps.
- Claims point at the files with a partial (⚠️) status: the documents exist,
  but they have not been externally reviewed and no migration code has landed.

## What this does not prove

- An external architecture or security review.
- That the proposed interfaces are the final ones; open questions are listed
  in the RFC.
- That any split of `main.rs`/`tui.rs` has happened; SA97-25 remains.
- That every threat is covered; the document is an inventory, not a proof.

## Verification

```bash
test -f docs/rfc/0001-module-boundaries.md
test -f docs/threat-model.md
./scripts/verify-claims.sh
```
