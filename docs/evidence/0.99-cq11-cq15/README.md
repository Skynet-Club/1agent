# 0.99 CQ11/CQ15 — eval case mapping and audit/replay projection evidence

Date: 2026-11-16
Scope: eval runs link their case id into the journal; exported Run nodes carry
audit/replay status and issue codes.
Result: **implemented and verified; a QA re-evaluation of the CQ matrix is
still required before claiming Yes.**

All fixtures are synthetic. No production journals, prompts, credentials or
secrets are included.

## 1. Tested tree

| Field | Value |
|---|---|
| Repository | `/home/skynet/1-agent` |
| Branch | `0.99-dev` |
| Slice tag | `0.99-cq11-cq15-export` |
| Changed files | `src/runtime.rs`, `src/eval.rs`, `src/ontology/export.rs`, `tests/ontology_cli.rs`, `docs/architecture.md`, `docs/0.99-handoff.md` |

## 2. Commands and results

```bash
cargo test --offline --lib
cargo test --offline --test ontology_cli
./scripts/verify.sh
```

| Command | Result |
|---|---|
| `cargo test --offline --lib` | **1118 passed, 0 failed, 4 ignored** |
| `cargo test --offline --test ontology_cli` | **27 passed, 0 failed** |
| `./scripts/verify.sh` | **passed** (fmt, clippy all-targets `-D warnings`, all tests, core-v3 8/8) |

Focused tests added:

- `runtime::tests::run_with_mode_and_client_request_id_links_the_journal` —
  the new entry point journals `ClientRequestLinked` without bypassing the
  fast-chat/repair behavior.
- `eval::tests::eval_runs_link_the_case_id_to_the_journal` — a real eval run
  with artifacts journals `eval:<case-id>`.
- `ontology::export::tests::run_exports_eval_case_id_and_audit_status` —
  `evalCaseId`, `clientRequestId`, `auditOk=false` for a duplicate tool call,
  `replayOk=true`, sorted issue codes, validator acceptance.
- `tests/ontology_cli.rs::eval_case_and_audit_status_are_exported` — CLI export
  and validate.

## 3. Acceptance mapping

| CQ criterion | Evidence |
|---|---|
| CQ11 eval case→run mapping | Eval journals `eval:<case-id>`; Run exports `evalCaseId` and `clientRequestId` |
| CQ15 audit/replay status | Run exports `auditOk`, `replayOk`, bounded `auditIssueCodes` |
| Old journals remain readable | Fields are optional; existing exports/tests stay green |
| Bad journals do not panic | Export test with a duplicate tool call; CLI validate test |
| Privacy | No prompt/tool content added; only ids, booleans and audit codes |

## 4. Explicit non-claims

- The CQ matrix has not been independently re-evaluated; CQ11/CQ15 moved from
  "not answerable" to "projected", but a QA verdict is still required.
- `clientRequestId` is exported for any client-linked run, not only eval; the
  `evalCaseId` field is derived only from the `eval:` prefix.
- Audit issue codes are capped at 16; the full findings remain in the journal.
- QG1/QG2/QG3 and the generic gated-tool approval card remain open.
