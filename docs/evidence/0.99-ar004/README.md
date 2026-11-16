# 0.99 AR-004 — action card binding and artifact-SHA drift evidence

Date: 2026-11-16
Scope: `ActionCard`/`ActionRequest` bind the runtime `action_id` and an
optional `expected_artifact_sha256`; matching, consumption and pending-card
reuse reject drift.
Result: **binding and drift rejection implemented at the browser card/store
layer; a generic gated-tool approval-card producer is still open.**

All fixtures are synthetic and live in temporary directories. No production
journals, prompts, credentials or secrets are included.

## 1. Tested tree

| Field | Value |
|---|---|
| Repository | `/home/skynet/1-agent` |
| Branch | `0.99-dev` |
| Slice tag | `0.99-ar004-card-binding` |
| Changed files | `src/browser/policy.rs`, `src/tools/browser.rs`, `src/tools/mod.rs`, `src/runtime.rs`, `docs/architecture.md`, `docs/0.99-handoff.md` |

## 2. Commands and results

```bash
cargo test --offline --lib
./scripts/verify.sh
```

| Command | Result |
|---|---|
| `cargo test --offline --lib` | **1107 passed, 0 failed, 4 ignored** |
| `./scripts/verify.sh` | **passed** (fmt, clippy all-targets `-D warnings`, all tests, core-v3 8/8) |

Focused tests added:

- `browser::policy::tests::approval_card_binds_action_and_expected_artifact_sha`
  — an approved card is consumed by the matching action id + SHA; a different
  SHA or action id returns `Mismatch`; the original request consumes once and
  a second take fails as not pending.
- `browser::policy::tests::pending_card_reuse_requires_the_same_action_and_expected_sha`
  — a changed SHA or action id creates a new pending card instead of reusing
  the old one.
- `browser::policy::tests::legacy_action_cards_without_binding_fields_are_readable`
  — cards without the new fields load with an empty action id and no SHA.

The runtime now also copies a successful browser `approval_id` into the final
`ActionRecord.approval_card_id`, so an executed action points at the card that
authorized it.

## 3. Acceptance mapping

| AR-004 criterion | Evidence |
|---|---|
| Card binds action_id | Request/card fields; `matches_request` and dedupe compare it; test |
| Card binds expected artifact SHA | Optional field; `matches_request` and dedupe compare it; test |
| Drift after approval rejects execution | `take_approved` returns `Mismatch` for a changed SHA or action id; the card is not consumed |
| No reuse | Consumed card returns `NotPending`; table-level test |
| Authority remains the decision source | The card only satisfies an approval that the policy already required; no new allow path in Authority |

## 4. Explicit non-claims

- The browser tool currently supplies `action_id` but no
  `expected_artifact_sha256` (browser actions do not produce a pre-known
  artifact). The drift guard is enforced but inert for browser requests.
- A generic gated-tool approval card (creating a card on Authority
  `require_approval`, human approval, retry) is not implemented. The existing
  runtime gate still denies those calls; this slice prepared the card schema
  and matching semantics.
- `1agent action approve/deny/list` (AR-006) is not implemented; browser cards
  are approved with `browser approve`.
- AR-005 ontology nodes remain open.
