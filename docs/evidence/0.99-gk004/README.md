# 0.99 GK-004 — commitment lifecycle negatives evidence

Date: 2026-11-16
Scope: commitment id/description validation, one-ticket binding, terminal
commitment states, and the interaction with goal closure.
Result: **implemented and verified; GoalKernel still needs independent QA and
the remaining G1 checks.**

All fixtures are synthetic and live in temporary directories. No production
journals, prompts, credentials or secrets are included.

## 1. Tested tree

| Field | Value |
|---|---|
| Repository | `/home/skynet/1-agent` |
| Branch | `0.99-dev` |
| Slice tag | `0.99-gk004-commitments` |
| Changed files | `src/goal.rs`, `src/surface_service.rs`, `docs/architecture.md`, `docs/0.99-handoff.md` |

## 2. Commands and results

```bash
cargo test --offline --lib
./scripts/verify.sh
```

| Command | Result |
|---|---|
| `cargo test --offline --lib` | **1113 passed, 0 failed, 4 ignored** |
| `./scripts/verify.sh` | **passed** (fmt, clippy all-targets `-D warnings`, all tests, core-v3 8/8) |

Focused test added:

- `goal::tests::commitment_lifecycle_negatives_fail_closed` — empty
  id/description, duplicate id in the same or another goal, ticket link is
  one-shot (same ticket idempotent, different ticket rejected), empty ticket,
  unknown commitment, closing to `pending` rejected, closed commitment cannot
  be closed again or re-linked, and a commitment cannot be added to a
  non-active goal. Replay-snapshot equality is asserted.

## 3. Acceptance mapping

| GK-004 criterion | Evidence |
|---|---|
| `pending -> kept|missed|cancelled` terminal states | Only pending commitments can close; repeated close fails |
| Lifecycle negatives | Test above; errors are typed and persisted history is unchanged |
| Ticket binding | One ticket per commitment; same ticket idempotent; different ticket rejected |
| Cross-goal safety | Commitment ids are globally unique, so another goal cannot reuse or close the same id |
| Abandoned closure semantics | Existing GK-003 test: every pending commitment is enumerated and cancelled in the same append |

## 4. Explicit non-claims

- GoalKernel is not accepted as a whole; G1 still needs an independent QA pass
  and the ontology Goal/Commitment nodes (GK-006).
- `1agent goal ...` CLI (GK-005) is still not implemented.
- A commitment has no owner field beyond its goal; per-person commitment
  ownership remains future work.
