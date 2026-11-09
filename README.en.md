# 1Agent

> **Ask what it is for before reaching for the drill.**

**A local-first, evidence-accountable Super-Agent runtime.**

[![Status: R1 release candidate](https://img.shields.io/badge/status-R1%20release%20candidate-orange)](#current-status)
[![Platform: Linux x86_64](https://img.shields.io/badge/platform-Linux%20x86__64-blue)](docs/ecosystem.en.md)
[![License: MIT](https://img.shields.io/badge/license-MIT-green)](https://opensource.org/licenses/MIT)
[![Rust 2024](https://img.shields.io/badge/rust-2024-orange)](https://www.rust-lang.org/)

[简体中文](README.md) · **English**

1Agent is not a thin chat wrapper around a model. It makes **task, evidence, authority, execution, verification and audit** one replayable chain. It clarifies why it is doing something before deciding what to do. It can analyze; once connected to a controlled execution substrate, it can also execute. Every step leaves a verifiable record.

Most agents hand you a polished drill recommendation.  
1Agent first asks: are you drilling to run a cable, or to hang a picture?

> **This project is under active development.** 0.95.0 is an R1 release candidate, not a finished product. Testing, feedback and contributions are welcome.  
> Join [Skynet-Club](https://skynet.club) to help build 1Agent and its open ecosystem.  
> Current Linux pre-release: [v0.95.0-rc.1](https://github.com/Skynet-Club/1agent/releases/tag/v0.95.0-rc.1).

---

## Why 1Agent

| Principle | What it means |
|---|---|
| **Ask for the purpose** | Distinguish goals from means; ask the one question that can change the plan |
| **Evidence first** | Journals, receipts and replay are product primitives; “done” without a journal is not done |
| **Humans stay in control** | Read-only by default; writes, network and execution require explicit authority; irreversible actions need confirmation |
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
- **Local memory**: four layers, scope/taint/TTL, encrypted payloads, soft/hard forget.
- **Skill lifecycle**: SKILL.md, versions, scanning, static evaluation, runtime canary and rollback.
- **Long-lived context**: goals/commitments, deterministic attention basics, personal context graph and delegation profiles.
- **Self-summary and evolution**: reflection/lesson, evolution proposals, ledger, human approval and rollback.
- **Model routing**: deterministic and OpenAI-compatible providers, cost/latency ledger, shadow execution.
- **Audit**: hash-chained journal, replay, receipts, query and export.
- **Optional execution**: with Skynet Workshop, controlled Session/sandbox execution and Artifact verification.

**Not available today**: browser tooling, generic subagents, MCP client, a complete always-on daemon, and Bot.  
These are listed honestly in [Status & Roadmap](docs/status.en.md).

---

## Quick start (Linux x86_64)

> **Linux x86_64 is the only validated and published platform.** Windows is not tested and is not published; macOS, Android and iOS are planned.

Download the Linux archive from Releases:

```bash
tar -xzf 1agent-0.95.0-linux-x86_64.tar.gz
cd 1agent-0.95.0
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
| Skynet Workshop execution | **R&D / integration** | Optional execution substrate; W-Gate not formally closed |
| Browser / computer-use | **Not implemented** | Roadmap in [Status & Roadmap](docs/status.en.md) |
| Generic subagents | **Not implemented** | Only bounded Workshop Master→Worker scheduling exists |
| Always-on daemon / Routine | **R&D** | G2 scope |
| 1A Bot | **R&D** | Persona layer; 1Agent does not depend on it |
| MCP / tool connectors | **Not implemented** | Tool ecosystem work |

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

The future is open and iterative: browser, subagents, always-on autonomy, tool ecosystem, Workshop and Bot have a public capability plan. Each stage advances through testable milestones and evidence. Plans can be discussed and revised; the roadmap is not a one-time promise.

---

## Security and privacy

- Read-only by default; writes, network and execution require explicit authority.
- Keys are referenced through environment variables or local key files and are not written to journals, reports or configuration.
- Exports exclude key material; restore and migration are checked with SHA-256.
- See repository [`SECURITY.md`](SECURITY.md), [`PRIVACY.md`](PRIVACY.md) and [Design Philosophy](docs/design-philosophy.en.md).

---

## License

[MIT](https://opensource.org/licenses/MIT). Third-party notices and license inventory are included in release packages.
