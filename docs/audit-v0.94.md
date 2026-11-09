# v0.94.0 1A Production-Control Audit

Date: 2026-11-03

## Verdict

The phased production-control surface is implemented and passes the available automated,
CLI, workflow and PTY gates. This verdict covers the command architecture, Master lifecycle,
supervision, Artifact acceptance, context/cost controls and local session/workspace tools.

It does not claim fresh external Workshop measurements, authenticated multi-runtime success
rates or a human-productivity study in this closing run.

## Requirement matrix

| Requirement | Evidence |
|---|---|
| Core/Master command separation | command scope tests and standalone rejection tests |
| Lifecycle-aware Master drawer | stage tests plus line and raw PTY |
| `/` command panel replacement | replace/close unit test plus `/config` -> `/cost` -> `Esc` PTY |
| Selectable command submenus | parent/child unit test plus `/workspace` root-selection PTY |
| Chinese operator presentation | grouped-status tests plus DeepSeek-route `/status` PTY |
| Executable command palette | all-command action coverage tests plus `/help` -> `/status` -> `上下文` PTY |
| Mission/plan/approval | digest-bound session tests and full workflow test |
| Assignment/dispatch | deterministic assignment and fake Workshop dispatch tests |
| Leases/idempotency | Master coordination duplicate/stale-generation tests |
| Supervision/human input | marker, response and state-observation tests |
| Pause/resume/cancel/takeover/retry/replan | journaled control tests and PTY |
| Artifact and SHA-256 verification | missing file, criterion gap, digest drift and verifier separation tests |
| Master evidence/audit/report | deterministic view tests plus verified workflow |
| Token/context control | compaction, bounded facts, headroom and config tests |
| Provider usage/cost ledger | pricing fallback, aggregation, TUI rendering and PTY tests |
| Cross-runtime capability evidence | manifest validation, fake pass/fail/block, hash-only output, CLI E2E |
| Session/workspace/preset/export | metadata, safe path, trash, non-overwrite and CLI E2E tests |

## Automated gates

```text
cargo fmt --all -- --check                  passed
cargo test --all-targets                    passed
cargo clippy --all-targets -- -D warnings   passed
RUSTDOCFLAGS="-D warnings" cargo doc        passed
cargo build --release                       passed
git diff --check                            passed
```

Test totals:

```text
library: 231 passed
main/TUI: 20 passed
CLI E2E: 16 passed
other offline integration suites: all passed
loopback-only provider test: ignored by design in managed sandbox
```

Release binary digest:

```text
sha256 864ffaf035af9e205a09a875ddb75319e13709bc009a7a08d3cf6bbfe52f606b
```

## Workflow evidence

Automated TUI workflow:

```text
mission -> PlanProposal -> digest approval -> deterministic assignment
-> fake Workshop dispatch -> Running -> ArtifactReady
-> independent verification -> Verified -> evidence/report/audit
```

PTY evidence:

```text
phase 4: board/watch/attention/respond/pause/resume/takeover/cancel/timeline
phase 5: artifacts/verify/evidence/audit/report, final verified, 7-event audit pass
phase 6: headroom/config/compact/workspace/preset/session fork/export
post-phase 6: /cost ledger and persisted compact threshold on a legacy session
command panels: /config replaced by /cost with no prior-panel residue; Esc restored conversation
workspace submenu: /workspace -> 设置工作区 -> root 2 applied to cwd and persisted session state
operator panels: /status grouped Chinese fields; long panels open at top and scroll to the end
command palette: /help selected /status; /status selected 上下文 and executed /context show
capability probe: Pi/Hermes/Claude/Codex/DeepSeek smoke plus report integrity
```

## Residual limits

- External Workshop and multi-runtime capability measurements were not repeated during
  this closing run; earlier live evidence remains historical context.
- `/status` character headroom is an estimate, while `/cost` uses journaled provider token
  usage and configured pricing metadata; neither is a general currency claim.
- The ignored loopback provider test requires execution outside the managed sandbox.
- Theme, Vim, key remapping and pets are excluded because they do not improve production
  reliability or evidence quality.
- The working tree is intentionally uncommitted. No repository history was rewritten.
