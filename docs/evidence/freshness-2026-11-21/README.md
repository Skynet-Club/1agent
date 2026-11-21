# Fresh-artifact completion gate — evidence (2026-11-21)

Status: **fixed, tested, live-rerun verified**. Not a release; binary version
remains `1.0.0-rc.13`.

## Bug

The user's two real runs reused `/home/skynet/work/join-us/index.html`
(624,456 bytes, sha256 `fc7bec90…`) instead of producing a new page:

- `db5e814e…` — no `write_file`/`edit_file`; it verified and audited the old
  file, `acceptance_gate` accepted, final status `completed`.
- `163f2f00…` — same reuse; an early gate even recorded that the file was the
  previous run's and “本轮没有重新设计”, then a later gate accepted it.

Root cause: `CompletionGate::observe` accepted any `accepted` receipt without
checking that the receipt's HTML artifact was written or edited in this run.

## Fix

- `acceptance_gate` receipts for a page task are accepted only when every
  HTML path named in `expected_artifacts` / `artifact_checks` was written by a
  write tool in this run, or its mtime is at/after the run start (covers
  terminal-generated pages).
- A stale receipt is not stored; the run records `CompletionArtifactStale` and
  `GateFailure::StaleArtifact` downgrades a `completed` answer to
  `conditional` with a `[新鲜度门]` explanation.
- One fresh-artifact nudge tells the model to write the page instead of
  re-verifying an existing `index.html`.
- The page checklist now says: “Do not reuse or merely re-verify a pre-existing
  index.html; the page must be written or edited in this run.”

Code: `src/runtime.rs`, `src/journal.rs`.
Tests: `pre_existing_page_receipt_is_conditional`,
`fresh_page_write_after_a_stale_receipt_can_complete`.

## Gates

- `cargo fmt --check`: clean.
- `cargo clippy --offline --all-targets -- -D warnings`: clean.
- `cargo test --lib`: **1391 passed / 0 failed / 4 ignored**.
- Full `cargo test`: one known environment failure,
  `cli_tui_agents_refresh_uses_configured_profiles` (skynet-term 128 active
  session limit), same as the rc.13 checkpoint.
- `eval --suite evals/core.json`: **8/8 PASS**.

## Live rerun (exact user objective, final binary)

Binary: `~/.local/bin/1agent-freshness`, sha256
`2a5ffd24cb2e82f56c8b3362cefdd7da9d2e95755c06bfe89ee31ee2d61f7e8b`.

Run 2 started with both old artifacts present (`index.html` from 2026-11-20
and `join-v2.html` from the first verification run at 09:29):

- Wall clock: 180.9s; 30 model responses; 40 tool calls.
- Journal: `journals-run2/cde00be2-ccec-4940-a012-1dbfb317eee2.jsonl`.
- New artifact: `/home/skynet/work/join-us/join-mobile.html`, 34,729 bytes,
  sha256 `d5009acd9e49e3b7920edb516084f2abe031c453584c208357c2bbbef23f69b9`.
- Old artifact untouched: `/home/skynet/work/join-us/index.html`, 624,456 bytes,
  sha256 `fc7bec90dba5c9a1e34502c2e1c85d4a215dbf23a5045482661a1407d9f1ea1f`,
  mtime 2026-11-20 20:06:17.
- `write_file` created `join-mobile.html`; the accepted gate named only that
  path in `expected_artifacts`. No `CompletionArtifactStale` event.
- Browser audit: 390/768/1440, overflow 0, placeholders 0; the first two
  passes had 15 small tap targets, the final two had 0.
- Final status `conditional`, not `completed`: the model itself declined
  `completed` because the contract lacked a separate design-review criterion
  and `ui-ux-pro-max` / `unslop` / `lark-cli` were unavailable
  (`terminal_create` hit the skynet-term session limit). It named the blockers
  instead of reusing an old page.

The first verification run (binary `09266b7e…`) produced
`join-v2.html` (27,978 bytes, sha256 `494cad89…`) with the old `index.html`
also untouched; its evidence remains in `journals/` and `run.json`.

The user's TUI was running, so `cp` over `~/.local/bin/1agent` failed with
“text file busy”. The verified build was installed side-by-side, tested, then
`mv`'d over `~/.local/bin/1agent` (atomic replace; the running TUI keeps the
old inode). `sha256sum ~/.local/bin/1agent` now reports `2a5ffd24…`. The
previous binary is backed up at
`~/.local/bin/1agent-pre-freshness-2026-11-21`. A TUI that was already open
still runs the old code until restarted.

The user's TUI was running, so `cp` over `~/.local/bin/1agent` failed with
“text file busy”. The verified build was installed side-by-side, tested, then
`mv`'d over `~/.local/bin/1agent` (atomic replace; the running TUI keeps the
old inode). `sha256sum ~/.local/bin/1agent` now reports `2a5ffd24…`. The
previous binary is backed up at
`~/.local/bin/1agent-pre-freshness-2026-11-21`. A TUI that was already open
still runs the old code until restarted.

## Answer-fallback wording (same session follow-up)

The user's `needs_evidence` run ended with:

> 模型返回的答案结构无法修复，已降级为需要证据；本轮没有执行任何外部动作。

That was both jargon and false: the run had 14 tool calls. `unrepairable_answer`
now receives the run's tool count and completion gate and says what happened:

> 模型最后一次回答的格式不对，自动重试也没能修好，所以这次不能把它的结论
> 当成可信交付，已降级为「需要证据」。本轮实际调用过 14 次工具（含失败），
> 这些动作没有回滚。本轮核验过这些文件：…；这可能是已有文件，不代表本轮
> 重新做了交付。可以重试，或者把任务范围缩小后再说一次。

Tests: `unrepairable_answer_reports_what_actually_ran`,
`irreparable_answer_output_becomes_needs_evidence`. Binary rebuilt as
sha256 `98be7004…` and installed at `~/.local/bin/1agent`.

## Honest limits

- A write of identical bytes to an existing path counts as fresh. Detecting a
  byte-for-byte “no redesign” would require a pre-run snapshot of every HTML
  path; not implemented.
- The gate is scoped to page-shaped tasks (`looks_like_page_task`). A non-page
  task that reuses an old artifact is still governed only by the normal
  acceptance receipt.
