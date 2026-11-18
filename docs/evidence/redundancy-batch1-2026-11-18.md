# Redundancy cleanup — Batch 1 (2026-11-18)

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

## Not in this batch

- 6 unwired modules (`source_adapter`, `supervision`, `provider_comparison`,
  `shadow_provider`, `ops_guard`, `workshop_worker`) and `skill_candidate`
  remain pending a roadmap decision.
- Duplicate persistence helpers (`write_private_atomic`, `open_with`,
  `save/license`) remain pending Batch 3.
- Legacy path/schema compatibility and `main.rs` structure were not touched.
