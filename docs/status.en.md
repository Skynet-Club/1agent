# Status & Roadmap

> Current version: **0.95.0 R1 release candidate**  
> Platform: **Linux x86_64**  
> Conclusion: **usable and evaluable, but not production-stable.**

[简体中文](status.md) · **English**

## 0. Still developing; welcome to build with us

- **Under active development**: 0.95.0 is not finished; interfaces, configuration and boundaries may change.
- **Testing and contributions welcome**: developers, testers, researchers, security specialists, product and design partners can try it, open issues/PRs, join red-team and real-workflow validation, and join [Skynet-Club](https://skynet.club).
- **The future is open**: browser, subagents, always-on autonomy, Workshop and Bot have a public capability plan; each stage advances through testable milestones and evidence. Plans can be discussed and revised; the roadmap is not a one-time promise.

## 1. What this version is

0.95.0 is an **R1 release candidate**:

- not 1.0;
- not production-stable;
- not a complete Super-Agent;
- not multi-tenant or organization-grade;
- not Bot.

It can already:

- start TUI / headless / local service;
- handle tasks, evidence, authority, approval and audit;
- use local memory, skills, goals/commitments, delegation and context graphs;
- connect to OpenAI-compatible models;
- install, initialize, export, restore, upgrade and uninstall;
- produce a Linux release archive with SHA256SUMS and provenance.

## 2. Why it is not production-stable

### 2.1 External gates remain open

- A-Gate has not been fully closed;
- S-Gate has not passed;
- external IdP, organization roles and third-party audit are unfinished;
- the Workshop full release gate is unfinished;
- a real cross-day/cross-week long-horizon task is still missing;
- system install/uninstall and retention automation still need acceptance.

### 2.2 Capability boundaries are visible

- no browser/computer-use;
- no generic subagents;
- no MCP client;
- no complete daemon/Routine/Trigger/Watchdog;
- no multi-tenant or organization audit;
- no official macOS/Windows support;
- no Android/iOS app.

### 2.3 The version itself is a candidate

- interfaces, configuration and data formats may change;
- real-model quality depends on providers and configuration;
- complex long-horizon success rates need more real evidence;
- third-party audit, production signing and approval are unfinished.

## 3. Platform release cadence

| Platform | Status | Plan |
|---|---|---|
| **Linux x86_64** | Validated platform | Stabilize install, upgrade, execution and audit |
| **Windows x86_64** | Internal build, not published | Untested; no download or support claim |
| **macOS** | Planned | After Linux stabilizes |
| **Android / iOS** | Planned | With Bot/mobile |
| **Linux ARM64** | Not released | Separate build/validation |

> Only Linux x86_64 release artifacts are supported.  
> Windows builds are internal validation only.

## 4. Roadmap

### R1: core runtime closure (current)

- close A-Gate/S-Gate;
- external IdP, third-party audit, Workshop release gate;
- cross-day long-horizon task;
- retention, organization audit, production signing/approval;
- Linux install/upgrade/export/uninstall acceptance.

### R2: action layer

- `web_fetch` read-only web retrieval;
- shell/write/edit/git/test tool matrix with authority and sandbox;
- MCP/connectors;
- logical subagents for read-only research/comparison/verification;
- Workshop browser/computer-use productization (WS-13).

### R3: standalone Super-Agent

- cross-day/cross-week tasks;
- goal/commitment dependencies and evidence;
- full attention;
- skill capture→evaluation→activation→rollback;
- self-summary/evolution closure;
- bounded subagents mapped to Workshop Workers;
- S-Gate.

### G2 / R4+

- daemon / Routine / Trigger / Source / Watchdog;
- automatic evolution and skill library;
- multi-Bot, multi-tenant, organization audit;
- mobile and ecosystem.

> The roadmap is a direction, not a date commitment. Scope changes follow the plan index and governance records.

## 5. R&D status

| Capability | Status | Target |
|---|---|---|
| Stable Workshop execution | R&D/integration | R1/R2 |
| Browser / computer-use | Not implemented | R2 |
| Generic subagents | Not implemented | R2/R3 |
| MCP client | Not implemented | R2 |
| Complete daemon/Routine | Not implemented | G2 |
| Full Attention/Digest | R&D | G2 |
| Automatic evolution/skill marketplace | R&D/planned | G2/G3 |
| Multi-tenant/organization audit | Not implemented | R6 |
| Bot (persona/channels) | R&D | R4+ |
| Official Windows support | Internal build | Later |
| macOS | Planned | Later |
| Android/iOS | Planned | Bot phase |

## 6. What the release contains

The public GitHub pre-release contains:

- `1agent-0.95.0-linux-x86_64.tar.gz`;
- `SHA256SUMS`;
- `linux-x86_64.json` (Linux provenance).

Inside the Linux archive:

- the `1agent` binary;
- `scripts/install.sh`;
- `LICENSE`, `LICENSES/`;
- `SECURITY.md`, `PRIVACY.md`;
- `THIRD_PARTY_NOTICES.md`, `THIRD_PARTY_LICENSES.md`;
- `docs/`, `evals/`, `configs/`;
- `SHA256SUMS` for package contents;
- Linux provenance.

Verify:

```bash
1agent --version
1agent doctor
sha256sum -c SHA256SUMS
```

> The public pre-release does not publish Windows or the dual-platform manifest.  
> The dual-platform manifest remains an internal verification artifact.

## 7. Claim boundaries

We can say:

- “1Agent is a local-first, auditable agent runtime”;
- “this is an R1 release candidate”;
- “Linux x86_64 is the validated platform”;
- “optional Workshop execution is R&D and not yet stable”;
- “browser, subagents and always-on autonomy are still R&D”.

We must not say:

- “production-stable”;
- “the general Super-Agent is complete”;
- “it can operate any website”;
- “Windows/macOS/Android/iOS are supported”;
- “multi-tenant/organization audit is available”;
- “it has passed a third-party security audit”.

---

[Back to README](../README.en.md) · [Design Philosophy](design-philosophy.en.md) · [Ecosystem](ecosystem.en.md) · [Release Notes](release/RELEASE_NOTES.en.md)
