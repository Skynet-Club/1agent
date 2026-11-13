# 1Agent External Audit Package (Review Draft)

Status: **review draft — not an audit result and not an A-Gate pass**
Snapshot: local `main` at commit `d78759e` (2026-11-12)
Product: 1Agent / first-principles-agent, version 0.96.1
Platforms: Linux x86_64 verified; Windows compile-level only; macOS not verified

This package gives an external reviewer the contracts, evidence and known
limitations needed to plan an A-Gate/W-Gate or security review. It does not
claim that the review has happened.

## 1. Scope

- Local runtime: CLI, TUI, journal, checkpoint/resume, providers, tools,
  permissions, memory, skills, evaluation and release scripts.
- Historical R1 A-Gate gap list:
  `Skynet-Docs/Skynet-Club/III/01-1Agent/III-01-1Agent-A-Gate-差距清单.md`
  (2026-11-08 estimate: ~89% across 14 work packages, 0 packages closed,
  4/13 exit boxes checked; this estimate predates the 0.97 changes and must be
  re-scored).
- Out of scope for this snapshot: third-party security/compliance audit,
  macOS/Windows runtime, external IdP, production multi-tenant deployment.

## 2. Evidence index

| Gate | Evidence | Status |
|---|---|---|
| M4 crash recovery | 2h crash-injection soak: 28,552 cycles / 7,200.20 s, exit 0; raw output in `docs/evidence/crash-recovery-soak-2026-11-12/` | Local pass (soak); no real-crash aggregate |
| M5 replay | `docs/status-v0.97.md`; structural replay on completed local journals | Local pass |
| M6 security | `docs/threat-model.md`; local red-team/security tests; no external audit | Not externally verified |
| M13 audit completeness | `AgentAnswer::validate` on local journals | Local pass |
| M15 TUI frame p95 | release p95 1.26 ms on a 200-entry transcript (TestBackend) | Local pass |
| M16 1M journal incremental | 4.7 MB journal: full 31.8 ms, incremental p95 9.4–16.1 µs | Local pass |
| M17 compacted replay | forced-compaction fixture replay/audit 100% | Local pass |
| M1/M2/M3 | 8h run finished: 136/280 completed (48.57%), 238/280 replay-verified (85%), 280/280 crash-free; completion and replay gates failed | **Gate failed**; no human calibration |
| M7 TTFT | DeepSeek `deepseek-flash`, earlier n=3 p50 1.08 s / p95 2.48 s; new n=17 partial p50 1.31 s / p95 2.26 s | Small/partial sample |
| M19–M23 | 40-prompt golden set and answer-quality board exist; blind labels absent | Board only |

Claims: **223 total, 200 verified, 15 partial, 8 no evidence** as of this
snapshot (`docs/claims.md`, `./scripts/verify-claims.sh --run`). README claims
are not mapped one by one.

## 3. Security posture (local)

- Execution and file writes are enabled by default for local CLI/TUI;
  `--read-only` disables them. Side effects still pass the permission model,
  Authority/Policy checks and tests.
- `docs/threat-model.md` lists P0/P1 boundaries, mitigations and residual gaps;
  it is a self-assessment, not an external audit.
- Runtime kill switch: `~/.1a/KILL` (legacy `.1a/KILL`) denies write-capable
  tools with `kill_switch_active`; read tools stay available. It is not an OS
  sandbox.
- Known gaps: no OS-level process/network sandbox on all paths; same-UID
  processes are unrestricted; no external IdP; no journal hash chain across
  processes; browser/CUA execution and remote MCP transports have their own
  documented limits.
- Credentials: provider keys are read from environment/auth files; tests assert
  secrets are not printed. Reviewers should verify this on their own host.

## 4. Reproduction

```bash
cargo fmt --check
cargo test --offline
cargo clippy --all-targets --offline -- -D warnings
./scripts/ci.sh
./scripts/verify-claims.sh --run
./scripts/release-check.sh
./scripts/soak-recovery.sh                 # deterministic crash-injection
./scripts/soak-recovery.sh                 # 2h: SOAK_RECOVERY_SECONDS=7200
```

Real-provider benchmark (requires a provider key; do not put keys in logs):

```bash
export PI_DEEPSEEK_API_KEY="$(jq -er '.deepseek.key' ~/.pi/agent/auth.json)"
./target/release/first-principles-agent eval --suite evals/real-tasks-v1.json \
  --config configs/pi-deepseek.json --allow-network --timeout-seconds 240 --json
./target/release/first-principles-agent benchmark-run \
  --suite evals/real-tasks-v1.json --runs 1 --config configs/pi-deepseek.json \
  --allow-network --timeout-seconds 240 --json
```

Journal and rollback checks:

```bash
1agent audit --journal PATH --json
1agent metrics --journal-dir DIR --json
1agent doctor
```

The 0.96.1 published binary cannot read a 0.97 journal (first failure:
`RunStarted/max_cost_micros`; `model_request_delta` is also unknown). Finish or
archive new journals before downgrading. See
`docs/evidence/rollback-drill-2026-11-12.md`.

## 5. Known limitations for the reviewer

- M1/M2/M3: the 8h run failed at 48.57% completion and 85% replay; its
  failures are model-contract violations, not crashes. The 8-task suite is
  short and calibrated locally. M19–M23 blind labels are external.
- M6/M18: no external security/compliance audit; P0 unknown.
- A-Gate: 14 work packages were ~89% estimated on 2026-11-08; no package was
  fully closed. Re-score after the 0.97 slice.
- W-Gate/P-Gate: macOS and Windows runtime behavior are not verified; Linux is
  the only verified platform.
- `ModelRequested`/checkpoint deltas make new journals one-way compatible.
- The TUI/daemon projections do not use the incremental journal cursor
  everywhere; `replay_events` does not parse request content.
- Some `eval` suites are contract fixtures, not model-quality evidence; the
  real-task suite still needs a human reviewer/calibration.

## 6. Reviewer checklist

- [ ] Rebuild from the stated commit and verify the binary hash.
- [ ] Run `ci.sh` and `verify-claims.sh --run` on a clean host.
- [ ] Confirm no provider secret appears in logs, journals or error output.
- [ ] Attempt unapproved write/terminal actions under `--read-only`, Authority
      and kill switch; confirm fail-closed.
- [ ] Replay a sample of completed journals and audit them.
- [x] 8h benchmark result captured (failed; see
  `docs/evidence/long-horizon-benchmark-2026-11-12.md`).
- [ ] Perform an independent red-team pass (prompt injection, SSRF, secret
      leakage, permission bypass).
- [ ] Record P0/P1 findings and an explicit pass/fail decision; this package is
      not that decision.

## 7. Contacts and ownership

- Owner: 1Agent CTO / Skynet-Club.
- Evidence root: `docs/evidence/`.
- Claims: `docs/claims.md`; status: `docs/status-v0.97.md`.
- Threat model: `docs/threat-model.md`; rollback: `docs/rollback-v0.97.md`.
