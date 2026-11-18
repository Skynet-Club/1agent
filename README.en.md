# 1Agent

> **Ask what it is for before reaching for the drill.**

**A local-first, evidence-accountable Agent runtime; 1.0.0 targets a Super-Partner.**

[![Status: R1 release candidate](https://img.shields.io/badge/status-R1%20release%20candidate-orange)](#current-status)
[![Platform: Linux x86_64](https://img.shields.io/badge/platform-Linux%20x86__64-blue)](docs/ecosystem.en.md)
[![License: MIT](https://img.shields.io/badge/license-MIT-green)](https://opensource.org/licenses/MIT)
[![Rust 2024](https://img.shields.io/badge/rust-2024-orange)](https://www.rust-lang.org/)

[简体中文](README.md) · **English**

1Agent is not a thin chat wrapper around a model. It makes **task, evidence, authority, execution, verification and audit** one replayable chain. It clarifies why it is doing something before deciding what to do. It can analyze; once connected to a controlled execution substrate, it can also execute. Every step leaves a verifiable record.

Most agents hand you a polished drill recommendation.  
1Agent first asks: are you drilling to run a cable, or to hang a picture?

> **This project is under active development.** Current code line: **1.0.0-rc.9** (2026-11-18, code-level release candidate, **not GA**). 0.99 code-level acceptance passed; Track Q is 99/100. Testing, feedback and contributions are welcome.  
> 1.0 status and blocked gates: [status-v1.0](docs/status-v1.0.md) · [code-complete plan](docs/1.0-code-complete-plan.md). External audit, real Windows/macOS GA, marketplace and R1–R9 metrics remain **blocked**; no GA claim.  
> This line includes the 0.98 real-world/presence work and the 0.99 Goal/Action/Delegation kernels, plus QG1 goal-closure evidence and the Track Q safety/quality fixes.  
> Join [Skynet-Club](https://skynet.club) to help build 1Agent and its open ecosystem.  
> 1.0.0-rc.9 local candidate notes: [release notes draft](docs/release/RELEASE_NOTES-1.0.0-rc.9-draft.md)  
> 0.97 features and entry points: [RELEASE_0.97.en.md](docs/release/RELEASE_0.97.en.md) · [feature matrix](docs/release/FEATURES-0.97.md)

---

## Why 1Agent

| Principle | What it means |
|---|---|
| **Ask for the purpose** | Distinguish goals from means; ask the one question that can change the plan |
| **Evidence first** | Journals, receipts and replay are product primitives; “done” without a journal is not done |
| **Humans stay in control** | Local CLI/TUI run at full power by default: writes, terminal execution and network are on; `--read-only` disables writes and `--no-network` disables network; irreversible actions still need confirmation |
| **Local-first** | One runtime home (`$HOME/.1a`); data can be exported and restored; no cloud account required |
| **Provider-neutral** | Deterministic provider and OpenAI-compatible providers; models are replaceable and routing is explainable |
| **Reversible** | Soft/hard memory forget, skill rollback and evolution rollback; changes can be undone |
| **Simplicity first** | Occam output contract: conclusion first, then evidence; no padding |
| **One voice** | Workers do not talk to the user; 1Agent assembles the final answer |

Read the full design trade-offs in [Design Philosophy](docs/design-philosophy.en.md).

---

## What works today

- **Interactive TUI**: conversation modes, `/` command drawer, categories and recent commands, transcript scrolling, copy and run status.
- **Headless runs**: `1agent run` for scripts, CI and batch work.
- **Evidence-backed tasks**: task graph, tickets, mandates, approvals, side effects, reports and receipts.
- **Local memory and user model**: four layers, scope/taint/TTL, encrypted payloads, soft/hard forget; `UserModel` 2.0 records preferences, boundaries, autonomy, voice mode and memory consent (inspect with `1agent user-model`).
- **Skill lifecycle**: SKILL.md validation, versions, execution eval, blind A/B comparison, active lockfile, bwrap script sandbox, runtime canary and rollback.
- **Long-lived context**: goals/commitments, deterministic attention basics, personal context graph and delegation profiles.
- **Self-summary and evolution**: reflection/lesson, evolution proposals, ledger, human approval and rollback.
- **Model routing**: deterministic and OpenAI-compatible providers, cost/latency ledger, shadow execution.
- **Audit**: hash-chained journal, replay, receipts, paginated query and export.
- **Execution substrate**: with Skynet Workshop, controlled Session/sandbox execution and Artifact verification.
- **Tool surface**: `read_file` / `read_pdf` / `search_files`, `write_file` / `edit_file`, real PTY terminal, `fetch_url` / `web_search` / `browser_read`, CDP `browser` with action cards, stdio/HTTP MCP (dual-era, pinning, OAuth/CIMD) and `spawn_agent`.
- **Always-on skills**: `schedule` / `daemon` / `schedule health`, durable run ledger, systemd user unit + `sd_notify`, `--routine-skill`, SHA-256-gated `skill export/import` and Ed25519 bundle signing.
- **Observability**: `cost` by provider/model/task/tool, `digest` (runs/schedules/health/alerts) and a static HTML `dashboard`.

**Not available today on 0.97 stable**: computer-use clicks/screenshots, voice/image understanding, automatic watchdog escalation, mobile and Bot. On 0.98-dev these browser features exist behind action cards; bwrap/Landlock around Chrome, a remote OAuth server evidence run, adaptive red team and the release gates remain open.  
These are listed honestly in [Status & Roadmap](docs/status.en.md).

---

## Roadmap: Super-Agent 0.97–1.0

0.97.0 is still an auditable runtime at the R1-candidate stage, not an accepted
Super-Agent. The next steps are public and gated by metrics and evidence rather
than feature count:

| Version | Theme | Target window | Core outcome |
|---|---|---|---|
| **0.97.0** | Reliable Loop | 2026-11-14 (RC) | Long-horizon reliable closure: streaming, parallel read-only tools, incremental journal, crash recovery/checkpoint resume, verification gate, memory MVP, sandbox and a result-first answer contract; User Model 2.0, Trust & Repair, Value Report v0, cross-platform adapter layer |
| **0.98.0** | Real World | 2027-02-02 | Browser writes / computer-use, MCP expansion, skill learning and a resident MVP; Agent Inbox, desktop/chat surface, autonomy dial, partner voice, cross-platform Alpha |
| **0.99.0** | Multi-Agent Organization | 2027-03-16 | Master/Worker reliability, cost governance, cascade containment, shared team partner, external-audit preparation, cross-platform Beta |
| **1.0.0** | Trusted Super-Partner | 2027-05-11 | User model, Agent Inbox, external audit, SDK/API, team collaboration, cross-platform GA and ecosystem |

Dates are target windows, not unconditional promises. **If a metric gate does
not pass, we do not release and do not claim Super-Agent.**

---

## Quick start (Linux x86_64)

> **Linux x86_64 is the only validated and published platform.** Windows is not tested and is not published; macOS, Android and iOS are planned.

Download the Linux archive from Releases:

```bash
tar -xzf 1agent-0.97.0-linux-x86_64.tar.gz
cd 1agent-0.97.0
./scripts/install.sh --prefix "$HOME/.local"
export PATH="$HOME/.local/bin:$PATH"

1agent --version
1agent init
1agent doctor
```

Start the offline TUI (deterministic provider, no network or API key):

```bash
1agent tui
```

Connect a real model (example: DeepSeek-compatible endpoint; keep the key in an environment variable):

```bash
export DEEPSEEK_API_KEY="..."
1agent tui --config configs/example.json --allow-network
```

Headless runs:

```bash
1agent run --task "Compare option A and option B, including what would change the recommendation"
1agent run --task "..." --config configs/example.json --allow-network
```

See the full installation, provider, TUI, export/restore, upgrade and troubleshooting guide in [Manual](docs/manual.en.md).

---

## Current status

**R1 release candidate. Not 1.0. Not production-stable.**

| Capability | Status | Notes |
|---|---|---|
| 1Agent core runtime | **R1 candidate** | TUI/CLI, tasks, authority, memory, skills, audit, packaging |
| Linux x86_64 | **Validated platform** | Release archive, installer, SHA256SUMS and provenance |
| Windows x86_64 | **Not published (untested)** | Internal build only; no download, not a supported platform |
| macOS / Android / iOS | **Planned** | To follow the Linux release and, for mobile, the Bot work |
| Skynet Workshop execution | **R&D / integration** | Execution substrate; W-Gate not formally closed |
| Web read/search | **Partial** | `fetch_url` + `web_search` (Bing RSS, best effort); authenticated pages not implemented |
| Document reading | **Partial** | `read_pdf` extracts PDF text via `pdftotext`; image/scan OCR and DOCX/PPTX not implemented |
| Browser / computer-use | **Partial** | `browser_read` headless rendering; clicks, typing, screenshots and computer-use not implemented |
| Generic subagents | **Partial** | `spawn_agent` bounded child runs (depth 2, isolated journal/workspace) |
| Always-on daemon / Routine | **Partial** | intervals, file triggers, restart catch-up, `--routine-skill`, `schedule health` and failure alerts; watchdog escalation and restart deduplication not implemented |
| Cost / latency dashboard | **Partial** | `cost` by provider/model/task/tool and static HTML `dashboard`; tool-level token attribution not implemented |
| Skill runtime | **Partial** | skill tools, SHA-256 bundles and Ed25519 signing; shadow/canary and a remote marketplace are unfinished |
| MCP / tool connectors | **Partial** | stdio MCP client; HTTP/SSE and a tool marketplace not implemented |
| 1A Bot | **R&D** | Persona layer; 1Agent does not depend on it |

Full details: [Status & Roadmap](docs/status.en.md).

---

## Ecosystem

| Component | Role | Status |
|---|---|---|
| **Skynet Workshop** | Optional execution substrate: sessions, sandbox, leases, Artifacts, recovery | R&D / integration; not a stable public product yet |
| **skyget** | Independent large-file transfer tool: resume, SHA-256, content cache | Independently usable; not a hard dependency of 1Agent |
| **1A Bot** | Persona and companion layer: Web/IM/Voice, Companion Contract | R&D, not released |

See [Ecosystem & Platforms](docs/ecosystem.en.md).

---

## Documentation

- [Design Philosophy](docs/design-philosophy.en.md) — why purpose comes first and why evidence is a product primitive
- [Features](docs/features.en.md) — what works, what is experimental, what is not implemented
- [Manual](docs/manual.en.md) — install, initialize, providers, TUI, runs, export/restore
- [Ecosystem & Platforms](docs/ecosystem.en.md) — Workshop, skyget, Bot and platform status
- [Status & Roadmap](docs/status.en.md) — current version, platform plan and R&D items
- [Release Notes](docs/release/RELEASE_NOTES.en.md) — this pre-release

---

## Community

1Agent is still under active development. We welcome developers, testers, researchers, security specialists, product and design partners:

- **Test and report**: try the Linux package and report installation, TUI, task, memory, skill and audit issues;
- **Contribute code and docs**: submit issues/PRs for tests, providers, tools, scenarios and documentation;
- **Security and evaluation**: join red-team, isolation, long-horizon and real-workflow validation;
- **Join the community**: [Skynet-Club](https://skynet.club), to help improve the product, the plan and the ecosystem.

The future is open and iterative: computer-use, voice/image understanding, automatic watchdogs, a tool marketplace, Workshop hardening and Bot have a public capability plan. Each stage advances through testable milestones and evidence. Plans can be discussed and revised; the roadmap is not a one-time promise.

---

## Security and privacy

- Local writes, terminal execution and network access are enabled by default; `--read-only` disables writes, `--no-network` disables network; high-risk actions still require explicit authority.
- Keys are referenced through environment variables or local key files and are not written to journals, reports or configuration.
- Exports exclude key material; restore and migration are checked with SHA-256.
- See repository [`SECURITY.md`](SECURITY.md), [`PRIVACY.md`](PRIVACY.md) and [Design Philosophy](docs/design-philosophy.en.md).

---

## License

[MIT](https://opensource.org/licenses/MIT). Third-party notices and license inventory are included in release packages.
