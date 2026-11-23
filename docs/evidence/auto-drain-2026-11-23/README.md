# Auto-drain and fast default — 2026-11-23

This slice closes the two pending items from `docs/1.3-handoff-2026-11-23.md`:
drain the post-run acceptance queue automatically, promote fast delivery for
small new Act tasks, and nudge a re-verify before the last configured step.

Binary: final `~/.local/bin/1agent` sha256 `1da41f5dbfd4ba5535babae6…`.
The N=3 benchmark ran on `b200a848affb98367f31…`, saved as
`~/.local/bin/1agent-bench-auto-drain-2026-11-23`; the pre-slice `ef3f0d69…`
is saved as `~/.local/bin/1agent-auto-drain-2026-11-23`. The final binary adds
only the empty-read-root delivery fallback after the benchmark build.

## What changed

- **Runtime auto-fast** (`src/runtime.rs`): `with_auto_fast(true)` routes a run
  to fast mode only when all of these hold: interaction mode Act, task profile
  Fast, no session continuation, no injected context facts, not verify-only,
  not new-version, write permission on, and not a page deliverable. Pages,
  deep/standard tasks, continuations, capability-preflight-selected tasks and
  high-risk/authority runs keep Core 7/full. Explicit `--fast` still wins.
- **Audit**: every run journals `FastModeSelected {enabled, reason}`; the
  checkpoint stores `fast_mode` so resume rebuilds the same tool surface.
- **Auto-drain**: `1agent run` enqueues a fast/deferred delivery and immediately
  processes the queue; the TUI drains at startup and after every run, and
  enqueues a fast run before draining. `/status` counts remain the durable view.
- **Pre-budget nudge**: when an accepted receipt was invalidated by a later
  write and at most two configured steps remain, the runtime injects one
  internal re-verify/re-gate message. The post-budget reserve step remains the
  backstop. Tests: `receipt_invalidation_nudges_before_the_last_configured_step`.
- **Snapshot root** (`AgentRuntime::with_delivery_root`): the delivery snapshot
  now pins the task workspace rather than a broad configured read root. With
  `read_roots=["/home/skynet"]` the old snapshot hit the 16 MB cap and every
  fast receipt was conditional; the task workspace makes the snapshot complete.
- **Overrides**: `1agent run --no-fast` forces the standard loop; the TUI and CLI
  accept `ONEAGENT_AUTO_FAST=0`. `scripts/phase4_bench.py` passes `--no-fast`
  to the full/core7 arms and reads the auto-drained fast receipt.

## Gates

- fmt, strict clippy (`cargo clippy --all-targets -- -D warnings`).
- lib **1450 passed / 0 failed / 4 ignored** (three new tests).
- bin **71 passed / 0 failed**.
- core eval **8/8**, hard-gate failures 0.
- `scripts/verify.sh` still stops at the pre-existing
  `cli_tui_agents_refresh_uses_configured_profiles` failure; the same test
  fails on a clean checkpoint in this environment.

## Live CLI smoke

Command (isolated journals under this directory):

```bash
1agent run \
  --task "写 …/workspaces/smoke2/out.txt，内容为 hello from auto fast，并在回答里给出该文件的 sha256" \
  --config docs/evidence/answer-reserve-2026-11-23/1agent-8787.json \
  --journal-dir docs/evidence/auto-drain-2026-11-23/runs --max-steps 8
```

- Auto decision: `FastModeSelected {enabled: true, reason: "auto: small new act
  delivery"}`, tool surface exactly
  `[read_file, terminal_create, terminal_exec, write_file]`.
- 3.4 s wall, plain-text answer, queue drained in the same process:
  `验收 "accepted" · run dcd956fd… · manifest e3524ad4da31`.
- Snapshot root is the task workspace and not truncated; no failed receipt
  checks. Output: `cli-auto-fast.txt`; receipt: `receipt-smoke2.json`.
- Final installed binary `1da41f5d…`: the same shape without a terminal,
  `0f51b4f4-18ef-47d5-ada6-0af7b07fc38c`, 23 bytes, receipt `accepted`.

With `ONEAGENT_AUTO_FAST=0` the same task runs Core 7/full:

- `FastModeSelected {enabled: false, reason: "fast mode off"}`, 37-tool
  surface, `状态：完成`, no delivery queue. Output:
  `cli-auto-fast-disabled.txt`.

## Live TUI smoke

Driven through tmux with an isolated `ONEAGENT_HOME` and session directory:

- Task: write `out.txt` under `workspaces/tui` and report its sha256.
- Result: 2 steps / 1 tool / **1.7 s**, answer `有条件完成`.
- TUI notices: `交付已入队：run ed4be65f…` then
  `交付验收：新处理 1 条（accepted 1 / conditional 0 / rejected 0），失败 0 条`.
- Journal: `FastModeSelected {enabled: true, reason: "auto: small new act
  delivery"}`, four fast tools, snapshot root `workspaces/tui`.
- Receipt `accepted`, **0 failed checks**, ledger written under the isolated
  home. Output: `tui-auto-fast.txt`; verification: `tui-verification.txt`;
  journal: `runs-tui/ed4be65f-….jsonl`.

## Full N=3 rerun

`docs/evidence/auto-drain-n3-2026-11-23/` reruns
`evals/real-tasks-v1.json` with four arms (`full`, `core7`, `fast`, `pi`) at
N=3. The full/core7 arms pass `--no-fast`; the fast arm uses the installed
auto-drain binary.

- fast **24/24 neutral, 24/24 accepted receipts, 3.95 s, 3 calls, 2 tools,
  fixed 813, prompt 4,639**; **zero** `journal:terminal_write_unpinned` and
  **zero** failed receipt checks.
- full 23/24 neutral (one `checksum-file` miss where `terminal_create` hit the
  host's 128-session cap), core7 24/24 neutral, Pi 24/24.
- Full/core7 medians: 18.4 s / 16.4 s and fixed 8,972 / 4,478; Pi 5.46 s and
  fixed 595.

Details, per-case table and the environmental-failure explanation are in
`docs/evidence/auto-drain-n3-2026-11-23/README.md` and `results.json`.
