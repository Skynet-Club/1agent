# 0.99 AR-005 + DK-005 — action/policy/approval and failed-member ontology evidence

Date: 2026-11-16
Scope: export gated tool observations as `ActionInvocation`,
`PolicyDecision` and `Approval`; export failed/timed-out/cancelled spawns as
`SquadMember`.
Result: **implemented and verified; CQ12 SHA-history and CQ16 approver/time are
still not answerable from the journal.**

All fixtures are synthetic and live in temporary directories. No production
journals, prompts, credentials or secrets are included.

## 1. Tested tree

| Field | Value |
|---|---|
| Repository | `/home/skynet/1-agent` |
| Branch | `0.99-dev` |
| Slice tag | `0.99-ar005-ontology-actions` |
| Changed files | `src/ontology/export.rs`, `src/ontology/ids.rs`, `src/ontology/validate.rs`, `tests/ontology_cli.rs`, `docs/architecture.md`, `docs/0.99-handoff.md` |

## 2. Commands and results

```bash
cargo test --offline --lib
cargo test --offline --test ontology_cli
./scripts/verify.sh
```

| Command | Result |
|---|---|
| `cargo test --offline --lib` | **1112 passed, 0 failed, 4 ignored** |
| `cargo test --offline --test ontology_cli` | **25 passed, 0 failed** |
| `./scripts/verify.sh` | **passed** (fmt, clippy all-targets `-D warnings`, all tests, core-v3 8/8) |

Focused tests changed/added:

- `ontology::export::tests::gated_actions_and_failed_spawns_export_policy_and_squad_nodes`
  — denied and executed actions, policy decision, approval card, artifact
  link, failed spawn SquadMember without `childRunId`, session export and
  standalone journal export.
- `tests/ontology_cli.rs::cq04_successful_spawn_exports_squad_lineage` — the
  timed-out spawn is now a `SquadMember` with status `timed_out`; the run lists
  both members.
- `tests/ontology_cli.rs::cq06_needs_authorization_run_and_policy_profile_are_exported`
  — needs_authorization action exports policy outcome/profile and approval.
- `tests/ontology_cli.rs::policy_and_approval_metadata_is_exported_for_actions`
  — typed nodes exist and raw metadata keys/secrets remain absent.
- Validator now requires `ActionInvocation.status`, `PolicyDecision.outcome`
  and `Approval.status`, and closes new references (`forAction`,
  `hasToolCall`, `hasApproval`, `hasActionInvocation`, `partOfRun`).

## 3. Acceptance mapping

| AR-005 / DK-005 criterion | Evidence |
|---|---|
| ActionInvocation / PolicyDecision / Approval nodes | Export test and CLI tests; typed, linked to run/tool call/observation |
| No invented Approval | Node is only created from `approval_card_id`/`approval_id` metadata |
| Failed/timed-out spawns are SquadMember | CQ04 CLI test and export unit test; status preserved, `childRunId` optional |
| No failure marked completed | Status is read from metadata/observation; fallback maps TimedOut to `timed_out`, other failures to `failed` |
| Old journals unchanged | Exports without action metadata keep the old node set; full suite green |
| Privacy | Raw prompts/arguments/content and raw metadata key names are absent |

## 4. Explicit non-claims

- CQ12 ("which artifact SHA changed after approval?") remains partial: the
  exporter can show the card and the final artifact SHA, but the journal does
  not record the expected SHA or a SHA history.
- CQ16 remains partial: the journal records the card id but not the approver
  or approval/consumption timestamps. A card store query is not part of export.
- Failed spawns that were refused before a child started (depth/fanout) are
  not SquadMember nodes.
- `ontology/bindings.json` is unchanged; F9 binding coverage remains open.
