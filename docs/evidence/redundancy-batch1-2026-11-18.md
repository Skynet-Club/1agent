# Redundancy cleanup — Batches 1–2 (2026-11-18)

Scope: zero-behavior cleanup only. No product path, CLI, TUI, tool registry,
journal event or persisted format is changed.

## Method

Static scan of `src/` production code (test modules excluded):

1. Find functions with no references in production code.
2. Split into fully dead (no test/doc references) and unit-test-only.
3. Exclude whole modules reserved for Batch 2.
4. Re-check integration tests, `main.rs` tests, docs and the binary target.

## Changes

- Deleted **22 fully dead functions** (no product, test or doc reference).
- Marked **40 unit-test-only functions** `#[cfg(test)]`; their tests remain.
- Kept **7 functions used by integration tests** unchanged
  (`reconcile_side_effects`, `publish`, `from_graph`, `bind_actor`,
  `payload_contains`, `register_profile`, `has_boundary`).
- Kept `HostBinding::workshop_context` in production: `main.rs` tests use it
  across the lib/bin boundary, so it is not test-only.
- Removed the now-unconstructed `TranscriptKind::Metadata` variant.
- Removed imports that became unused.

Deleted functions: `status_summary`, `job_tasks`, `wait_for_event`,
`wait_for_download`, `plan_triage_constraints`, `with_authority`,
`assistant_metadata`, `default_unit_path`, `capability_snapshot`,
`default_option`, `job_status`, `secret_input_label`,
`session_picker_selected_id`, `setting_input_label`, `cancel_setting_input`,
`clear_pointer`, `default_root`, `export_document_sha256`, `resources_mut`,
`retrieve_shadow`, `standing_mut`, `unknown_skill`.

Test-only functions gated with `#[cfg(test)]`: `digest_provider_probes`,
`evaluate_provider_alerts`, `dispatch_ticket`, `split_runtime_cases`,
`add_subscription`, `new_reflection`, `new_lesson`, `register_worker`,
`assistant_query_result`, `ingest_trigger_gated`, `remove_subscription`,
`close_thread`, `renew_lease`, `run_suite`, `intersect_scopes`,
`planner_sees_untrusted`, `action_record`, `filtered_commands`,
`remove_worker`, `select_prompt_category`, `decide_for`,
`approve_ticket_as_subject`, `grant_standing`, `input_box_contains`,
`with_context_cache`, `with_depth`, `with_quota`, `agent_system_prompt_for`,
`can_remember`, `compact_messages`, `ensure_builtin_bindings_parse`,
`ensure_builtin_types_parse`, `identity_mut`, `ingest_dir`, `is_accepted`,
`is_frozen`, `notify_inbox_if_new`, `restore_soft`, `unbind`.

## Verification

| Check | Result |
|---|---|
| `./scripts/verify.sh` | **pass**; 1668 total, 0 failed, 13 ignored |
| claims | 264 checked (240 verified / 16 partial / 8 no evidence) |
| diff | 34 files, 63 insertions, 235 deletions (net -172 lines) |
| test count | unchanged; removed functions had no tests |

Raw: `verify-batch1-cleanup.log`.

## Batch 2 — unwired modules (2026-11-18)

After an explicit product decision, deleted six modules that had no reference
from the binary, runtime, integration tests or docs:

- `src/source_adapter.rs` (477 lines with tests)
- `src/supervision.rs` (467)
- `src/provider_comparison.rs` (386)
- `src/shadow_provider.rs` (301)
- `src/workshop_worker.rs` (276)
- `src/ops_guard.rs` (166)

Total: 2,073 lines including 908 test lines and 17 unit tests. Their `pub mod`
and glob re-export entries were removed from `lib.rs`. `skill_candidate` was
kept because `skill_lifecycle_e2e` and `a_gate_heldout` exercise it.

Verification: `verify.sh` **1651 total / 0 failed / 13 ignored** (17 fewer
module tests); claims 264 (240/16/8); HTTP surface e2e 1/1; TUI interaction
**pass** after rebuilding the release binary. Removed code remains recoverable
from git history before commit `e85d49a` (Batch 1) or the deletion commit.

## Batch 3 — shared atomic write (2026-11-18)

- Added `jsonl::write_private_atomic`: private temporary file, `O_NOFOLLOW`,
  mode `0600`, `sync_data`, rename, parent-directory `sync_all`, and temp
  cleanup on failure.
- Replaced the three identical copies in `evolution.rs`, `delegation.rs` and
  `worker_runtime.rs`; net -47 lines after the shared helper.
- Added `private_atomic_write_replaces_content_and_leaves_no_temp_file`
  (content replacement, mode 0600, no `.tmp` leftovers).

Inspection changed the original Batch 3 scope:

- `tool_approval`/`browser::policy` `save` and `lock` are **not** exact
  duplicates. They use `<name>.tmp-<pid>` vs random UUID, different lock-file
  names and different fsync points. Merging them would change live behavior,
  so they were left unchanged.
- `memory_journal`/`skill_journal` `open_with` differ only by event type and
  store verifier; extracting a generic would add coupling for ~30 lines. Left
  unchanged.

Verification: `verify.sh` **1652 total / 0 failed / 13 ignored**; claims 264
(240/16/8); lib tests 1206.

## Not in this round

- `skill_candidate` remains test-only; whether to wire it or remove it is a
  product decision.
- Tiny utility clones (`safe_id`, `now_ms`, `bounded`) were left in place;
  the shared helper would add a module for little benefit.
- Legacy path/schema compatibility and `main.rs` structure were not touched.
