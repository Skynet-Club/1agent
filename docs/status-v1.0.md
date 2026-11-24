# 1Agent 1.0 status — code-complete candidate

Status: **T7 local QA pass; external gates blocked; not GA**  
Date: 2026-11-24  
Scope: code-level 1.0 candidate + local quality gates. This page is a
measurement review, not a release announcement. A blocked gate is not zero; it
means the evidence does not exist on this host.

## 0. One-line status

0.99 QG1 is fixed (goal closure requires a stored verification record); the
rc.2 Track Q run is **99/100** with one named efficiency residual (`intro-20`).
The 1.0 code-level candidate passes the local software gates, and **T7 final
local QA passed on 2026-11-24** (`release/1.0` at `2db4333`, rc.12). rc.11 made
local TUI/`run` full-power by default (network and writes on, `--no-network` /
`--read-only` opt-outs), removed the stale-clipboard right-click paste, added
`/setting` plus eight TUI themes, repeat/pass^k evaluation, path-vs-command
parsing, capability compiler P0/P1/P2 and step-budget finalization. It packages
as Linux `1.0.0-rc.12` from tag `v1.0.0-rc.12`. External audit, real
Windows/macOS GA, marketplace responsibility and R1–R9 real-user metrics remain
**blocked**; no GA claim.

## 1. Local evidence

| Check | Current result | Evidence |
|---|---|---|
| `./scripts/verify.sh` | **T7 pass**; lib 1202, bin 61, core-v3 8/8, claims structure 264 | `docs/evidence/t7-2026-11-24/verify.log` |
| Track Q `agent-loop-v1` | **99/100** (rc.2); 0 hard-gate; audit/replay 100/100; rubric 20/20; residual `intro-20` efficiency | `docs/evidence/1.0-local-qa-2026-11-17/agent-loop-v1-rc2-2026-11-18/` |
| Default permissions | rc.3 full-power: network and writes on; `--no-network`/`--read-only` opt-outs; TUI honors configured `full-local` | `tui_network_permission_defaults_on_and_no_network_disables_it` |
| Right-click paste | removed; `Ctrl+V`/`Shift+Insert` only; PTY test confirms no marker after right-click | rc.4 PTY test |
| Settings and themes | `/setting` two-level panel with in-panel edit, 8 themes persisted in `appearance.theme`, session/global reset with backup+confirm | `setting_*` tests, settings-panel PTY, `tui_setting_menu_shows_values_and_switches_theme` |
| Evaluation depth | `eval --repeats` pass^k, per-case attempts/passes, process metrics and rule-based failure classes | `docs/evaluation.md`, `docs/evidence/eval-repeatability-2026-11-18/` |
| Capability compiler | Open manifest resolver + runtime preflight, `verify_artifact`, install proposal/approval, capability-path memory | `docs/capability-compiler.md`, `docs/evidence/capability-compiler-2026-11-18/` |
| QG1 goal closure | pass; negative cases covered | `docs/evidence/0.99-qg1-verification/README.md`, ADR-0147 |
| QG2 action chain | pass; reconciliation is on-demand | `docs/evidence/0.99-acceptance/README.md` §3 |
| QG3 spawn lineage | pass with named residuals | `docs/evidence/0.99-acceptance/README.md` §3 |
| SDK Python | 6/6 unit tests; health/openapi/agent-card/trust-center/surface | `sdk/python/test_client.py` |
| Windows GNU check/link | **pass**; `.exe` SHA-256 `535eb260…` | `docs/evidence/platform-1.0-2026-11-17.md` |
| macOS | **blocked** by host C toolchain (`ring/cc`) | same |
| HTTP surface e2e | **pass** | `docs/evidence/1.0-local-qa-2026-11-17/http-e2e.log` |
| TUI interaction | **T7 pass** (real provider; harness fixed to check menu controls, not the word “下一步”) | `docs/evidence/t7-2026-11-24/tui-interaction-2.log` |
| 5-cycle soak | **T7 pass**, 5 cycles | `docs/evidence/t7-2026-11-24/soak.log` |
| `release-check.sh` | **T7 pass** (fmt, clippy, tests, HTTP e2e, CI incl. `verify-claims --run`, TUI interaction) | `docs/evidence/t7-2026-11-24/release-check.log` |
| Local package | `1.0.0-rc.12` Linux tar, `reproducible=true`, archive SHA `649e66aa…`, binary `26fc0988…`; SHA256SUMS/version/install smoke pass | `docs/evidence/release-1.0.0-rc.12-2026-11-18.md` |

## 1a. T7 final local QA (2026-11-24)

T7 re-ran the runnable local gates on the frozen `release/1.0` worktree
(`2db4333`, `1.0.0-rc.12`) and recorded every pass / partial / blocked result
in `docs/evidence/t7-2026-11-24/README.md`.

- **Pass**: `verify.sh` (lib 1202, bin 61, core-v3 8/8), SDK 6/6, 5-cycle
  recovery soak, cross-platform required gates, `release-check.sh` including
  `ci.sh` / `verify-claims.sh --run`, and TUI interaction.
- **Partial**: Track Q is still the rc.2 **99/100** evidence (0 hard-gate,
  audit/replay 100/100, rubric 20/20); it was not rerun on this line in T7.
- **Not run**: 8-hour DeepSeek quality, by the explicit cost/time rule.
- **Blocked**: macOS arm64 (`ring/cc` host C toolchain) and all external gates.
- Two harness fixes were needed for T7 itself: rustfmt line wrapping in
  `tests/recovery_soak.rs`, a hermetic `--workshop-prefix` in the TUI agents
  test, and a greeting assertion that checks menu controls instead of the word
  “下一步”. No product behavior changed.

## 2. Hard gates (local vs blocked)

| Gate | Metric | Current | Status |
|---|---|---|---|
| M1 | Long-horizon success | 1h DeepSeek 97.08% historical; Track Q final 99/100; 8h DeepSeek quality not claimed | **Partial** |
| M4 | Crash recovery ≥95% | 2h crash-injection soak 28,552 cycles / 7,200.20s, exit 0 (0.97 code line); **T7 5-cycle refresh pass on rc.12** | **Local pass** |
| M5 | Replay 100% | 100/100 structural replay on the rc.2 Track Q journals | **Local pass** |
| M6 | Zero security incidents | No P0 in local review/tests; risk-12 repaired high-risk gate fixed; no external audit | **Local pass / external blocked** |
| M13 | Audit completeness 100% | Local `AgentAnswer::validate` and journal audit pass | **Local pass** |
| M17 | Compacted replay 100% | Forced-compaction fixture replays/audits | **Local pass** |
| M18 | External audit | 0.97 independent code review 0 P0; no third-party certification | **Blocked (external)** |

## 3. Blocked external gates

- SA100-68 third-party audit / SOC2 / ISO 42001 / EU AI Act mapping.
- SA100-70 real Windows/macOS GA hosts, code signing, notarization,
  install/upgrade/uninstall matrix.
- SA100-71 marketplace (signing/revocation/scanning/responsibility not ready).
- SA100-73 mobile (not blocking 1.0).
- R1–R9 real-user activation, retention, value-report adoption, unit economics.
- A-Gate/W-Gate external checklists and external IdP/org roles.

## 4. Known limits

- CQ3/CQ6/CQ7/CQ9/CQ12/CQ16 remain Partial in the 0.99 ontology matrix.
- The Track Q suite is one provider/model sample; do not claim statistical
  stability from one run.
- No 8h DeepSeek model-quality claim; the prior 8h local-model soak proves
  runtime stability only.
- `browser_read` Chrome SSRF and journal hash-chain tamper evidence remain
  residual security items from the 0.97 review.
- Public release history/attribution is still governed by the clean-release
  branch policy; `check-attribution.sh` fails on the development history.
