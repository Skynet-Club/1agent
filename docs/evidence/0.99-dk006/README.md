# 0.99 DK-006 — TUI squad lineage summary evidence

Date: 2026-11-16
Scope: read-only lineage line in the squad details preview.
Result: **implemented and verified; no layout or interaction change.**

All fixtures are synthetic. No production journals, prompts, credentials or
secrets are included.

## 1. Tested tree

| Field | Value |
|---|---|
| Repository | `/home/skynet/1-agent` |
| Branch | `0.99-dev` |
| Slice tag | `0.99-dk006-tui-lineage` |
| Changed files | `src/main.rs`, `docs/architecture.md`, `docs/0.99-handoff.md` |

## 2. Commands and results

```bash
cargo test --offline --bin first-principles-agent subagent_
cargo test --offline --lib
./scripts/verify.sh
```

| Command | Result |
|---|---|
| `cargo test --offline --bin first-principles-agent subagent_` | **2 passed** |
| `cargo test --offline --lib` | **1114 passed, 0 failed, 4 ignored** |
| `./scripts/verify.sh` | **passed** (fmt, clippy all-targets `-D warnings`, all tests, core-v3 8/8) |

Tests:

- `subagent_preview_reads_the_child_journal` now asserts the lineage line
  with session, parent run, spawn call and child run.
- `subagent_lineage_uses_dashes_for_legacy_records` — a legacy registry row
  renders `-` for every missing field.

## 3. Acceptance mapping

| DK-006 criterion | Evidence |
|---|---|
| Show parent session/run, spawn call, child run | Preview test |
| Missing fields are not invented | Legacy record test uses `-` |
| Read-only, layout unchanged | Only one preview line added; no renderer changes |
| Old registry rows readable | Fields are optional; test covers the empty case |

## 4. Explicit non-claims

- This is a details-preview line, not a new squad graph/list view.
- No filtering by session or goal is added; the registry remains root-pid
  scoped.
- 0.99 is not accepted; the CQ matrix and QG runs remain.
