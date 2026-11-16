# 0.99 — generic gated-tool approval card evidence

Date: 2026-11-16
Scope: Authority `require_approval` → persistent pending card → human approve/
deny → one-shot retry execution.
Result: **implemented and verified; the approval card is a human gate, not a
second policy engine.**

All fixtures are synthetic and live in temporary directories. No production
journals, prompts, credentials or secrets are included.

## 1. Tested tree

| Field | Value |
|---|---|
| Repository | `/home/skynet/1-agent` |
| Branch | `0.99-dev` |
| Slice tag | `0.99-tool-approval-card` |
| Changed files | `src/tool_approval.rs`, `src/lib.rs`, `src/runtime.rs`, `src/main.rs`, `tests/action_cli.rs`, `docs/architecture.md`, `docs/0.99-handoff.md` |

## 2. Commands and results

```bash
cargo test --offline --lib
cargo test --offline --test action_cli
./scripts/verify.sh
```

| Command | Result |
|---|---|
| `cargo test --offline --lib` | **1124 passed, 0 failed, 4 ignored** |
| `cargo test --offline --test action_cli` | **6 passed, 0 failed** |
| `./scripts/verify.sh` | **passed** (fmt, clippy all-targets `-D warnings`, all tests, core-v3 8/8) |

Focused tests added:

- `tool_approval::tests::pending_cards_dedupe_and_approval_is_consumed_once`
- `tool_approval::tests::changed_arguments_run_or_denial_require_a_new_card`
- `tool_approval::tests::expired_cards_cannot_be_approved_or_consumed`
- `tool_approval::tests::canonical_digest_ignores_object_key_order`
- `runtime::tests::approved_gated_tool_executes_once_after_human_approval` —
  first call creates a pending card and does not execute; approval lets a
  retry execute once, the ActionRecord carries the card id, a third call
  needs a new card.
- `runtime::tests::changed_arguments_require_a_new_approval_card` — changed
  args do not consume the old approval; the matching args still do.
- `tests/action_cli.rs::action_approval_cards_can_be_listed_approved_and_denied`
  — list/approve/deny, duplicate approval fails.

## 3. Acceptance mapping

| Criterion | Evidence |
|---|---|
| Authority remains the only decision source | Only the `require_approval` branch consults the store; `deny` is unchanged |
| Card binds action/run/tool/arguments | `ToolApprovalCard` and canonical arguments digest; tests |
| One-shot execution | `take_matching` marks Consumed; reuse returns NotFound |
| Changed arguments require re-approval | Runtime test |
| Expiry/denial/duplicate | Store and CLI tests |
| ActionRecord links the card | Runtime test asserts `approval_card_id` on the executed record |
| Human approval | `action approve` confirmation; `--yes` for explicit non-interactive use |

## 4. Explicit non-claims

- Expected artifact SHA is not bound on generic tool cards; the canonical
  arguments digest covers content changes. Browser cards keep their optional
  expected SHA field.
- The store is local state (`~/.cache/1agent-tmp/tool-approvals.json` or
  `ONEAGENT_TOOL_APPROVALS`); it is not a signed approval envelope.
- TUI runs do not configure an execution gate, so they do not create these
  cards; gated CLI runs do.
- Independent QA, QG1/QG2/QG3 and the 1.0 work remain open.
