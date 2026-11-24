# T7 — Final local QA on release/1.0 (2026-11-24)

Scope: the frozen `release/1.0` worktree at `2db4333`, version `1.0.0-rc.12`.
This is a local-QA report for the 1.0 code-complete candidate, not a GA
announcement. External gates remain blocked.

## Changes made while running T7

- `cargo fmt --all`: one rustfmt wrap in `tests/recovery_soak.rs` (newer
  rustfmt than the one used when the line was committed).
- `tests/cli_e2e.rs`: the TUI agents refresh test now passes a missing
  `--workshop-prefix`, so it deterministically falls back to the configured
  `test-worker` profile instead of the host's 64+ Workshop agents.
- `scripts/tui-interaction-check.sh`: the standalone greeting assertion now
  looks for the next-action menu controls (`↑/↓ 选择，Enter 执行，Esc 返回对话`
  or `/frame（默认）`), not any occurrence of the word `下一步`. A live model
  greeting can legitimately mention "下一步" in prose; the old substring check
  produced one false failure. No product behavior changed.

## Gate results

| Gate | Result | Evidence |
|---|---|---|
| `scripts/verify.sh` | **pass**; lib 1202, bin 61, core-v3 8/8, claims structure 264 | `verify.log` |
| SDK contract | **pass**; 6/6 | `sdk.log` |
| Recovery soak | **pass**; 5 cycles | `soak.log` |
| Cross-platform code gate | **pass for required gates**; macOS arm64 blocked by host C toolchain (`ring/cc`) | `cross-check.log` |
| `release-check.sh` | **pass** on rerun (fmt, clippy, tests, HTTP e2e, CI, TUI interaction) | `release-check.log`; first-run failure `release-check-first.log` |
| CI (`ci.sh`) | **pass**; commit `2db4333` (dirty), binary sha256 `133a2ab3…`; includes `verify-claims.sh --run` | CI log `~/.local/state/1agent-ci/logs/20261124T033000Z.log` |
| TUI interaction | **pass** isolated twice after the harness fix | `tui-interaction-2.log`, `tui-interaction-3.log` |
| Track Q quality | **partial / not rerun**; reuse the 0.99 evidence (99/100, 0 hard-gate), which is an earlier code line | `docs/evidence/0.99-acceptance/agent-loop-v1-final-2026-11-17/` |
| 8h DeepSeek quality | **not run**; requires explicit cost/time approval | — |
| rc.12 packaging | **done earlier**; Linux archive/SHA256SUMS/install smoke | `docs/evidence/release-1.0.0-rc.12-2026-11-18.md` |

## Status

- T7 local software gates: **pass** (with the Track Q partial and the macOS
  toolchain blocker named above).
- 1.0 remains a **code-complete candidate**, not GA.
- Blocked external gates: third-party audit/SOC2/ISO/EU AI Act mapping; real
  Windows/macOS GA/signing; marketplace; R1–R9 real-user metrics; A-Gate/W-Gate
  external checklists and external IdP/org roles.
