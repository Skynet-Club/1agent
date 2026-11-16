# 0.99 AR-006 — `1agent action show|verify` CLI evidence

Date: 2026-11-16
Scope: black-box CLI inspection of the hash-chained action log and
reconciliation against one run journal.
Result: **implemented and verified; generic gated-tool approval cards and TUI
lineage display remain open.**

All fixtures are synthetic and live in temporary directories. No production
journals, prompts, credentials or secrets are included.

## 1. Tested tree

| Field | Value |
|---|---|
| Repository | `/home/skynet/1-agent` |
| Branch | `0.99-dev` |
| Slice tag | `0.99-ar006-action-cli` |
| Changed files | `src/main.rs`, `tests/action_cli.rs`, `docs/architecture.md`, `docs/0.99-handoff.md` |

## 2. Commands and results

```bash
cargo test --offline --test action_cli
cargo test --offline --lib
./scripts/verify.sh
```

| Command | Result |
|---|---|
| `cargo test --offline --test action_cli` | **5 passed, 0 failed** |
| `cargo test --offline --lib` | **1113 passed, 0 failed, 4 ignored** |
| `./scripts/verify.sh` | **passed** (fmt, clippy all-targets `-D warnings`, all tests, core-v3 8/8) |

CLI tests:

- `action_show_and_verify_pass_for_matching_records` — JSON show output and
  successful verify with `checked=1`.
- `action_verify_fails_when_the_record_is_missing` — non-zero exit and
  `missing_action_record`.
- `action_verify_fails_on_status_or_policy_mismatch` — reports
  `status_mismatch` and `policy_mismatch`.
- `non_gated_journal_verifies_against_an_empty_log` — `ok`, `checked=0`.
- `action_show_rejects_a_corrupt_log` — non-zero exit, no panic.

## 3. Acceptance mapping

| AR-006 criterion | Evidence |
|---|---|
| `action show` reads the latest records | Show test prints `action_id`, status and run; `--action-id` filters |
| `action verify` runs reconciliation | Verify tests; JSON issues and human stderr |
| Negative cases exit non-zero | Missing record, mismatch, corrupt log tests |
| Non-gated journals are not misjudged | `checked=0`, `ok` |
| No writes | Both commands only read the log and journal |

## 4. Explicit non-claims

- There is no `action approve/deny`; generic gated-tool approval cards are
  still not created.
- `action show` does not verify the journal; use `action verify` for that.
- DK-006 (TUI squad lineage summary) is not implemented.
