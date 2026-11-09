# Ecosystem & Platforms

> 1Agent is an independent product, not an attached UI for another component.  
> Components below are labeled optional, independent or R&D; unfinished capabilities are not promised.

[简体中文](ecosystem.md) · **English**

## 1. Overview

| Component | Role | Relationship to 1Agent | Status |
|---|---|---|---|
| **1Agent** | Core agent runtime | The product | **R1 candidate**, Linux x86_64 |
| **Skynet Workshop** | Execution substrate: sessions, sandbox, PTY, leases, Artifacts, recovery | Optional; required for execution tasks | **R&D/integration**, not a stable public product |
| **skyget** | Local large-file transfer: resume, SHA-256, content cache | Independent tool, not a hard dependency | **Independently usable**, versioned by skyget |
| **1A Bot** | Persona and companion layer: Web/IM/Voice, Companion Contract | Future consumer of the 1Agent API | **R&D, not released** |

```text
1A Bot (persona/channels, R&D)
        │
        ▼
1Agent (tasks/memory/skills/authority/audit)  ← independent product
        │
        ├─ Skynet Workshop (optional execution substrate, R&D/integration)
        └─ skyget (optional large-file transfer, independent)
```

## 2. Skynet Workshop

### What it is

Workshop is 1Agent's optional execution substrate:

- Session / Pane / Job;
- Linux sandbox, PTY, process tree and timeout;
- resource leases, capacity and serialized writes;
- Artifact storage, path constraints and SHA-256;
- events, recovery, takeover and verification primitives.

### Current status

**R&D/integration, not a stable product.**

- Deterministic integration, a real LLM canary, Artifact verification, leases and recovery have engineering evidence;
- the W-Gate public release checklist is not fully closed;
- browser/computer-use remains research track WS-13;
- do not treat it as the only production execution backend yet;
- without Workshop, 1Agent remains an analysis/planning Super-Agent.

### What “executable” means

- **No Workshop**: analysis, planning and audit; default tools are read-only file tools;
- **With Workshop**: controlled tasks can run in an isolated Session and Artifacts are independently verified;
- “started” is not “completed”; exit 0 is not “verified”.

See repository `docs/` for the Workshop plan and the Super-Agent capability plan.

## 3. skyget

### What it is

`skyget` is an independent large-file transfer tool for:

- models, datasets, images and build artifacts;
- downloads/uploads that need resume, background execution and SHA-256 verification;
- shared content cache across agents/processes;
- avoiding large files in chat context or ordinary shell pipes.

### Current status

**Independently usable, not a hard dependency of 1Agent.**

- verified version in the current environment: `0.24.x`;
- has its own daemon, CLI, cache and ledger;
- the 1Agent package does not include skyget;
- small files and metadata can still use ordinary HTTP tools;
- large transfers should use `check` / `get` / `watch` / `pause` / `resume`.

```bash
skyget check <URL> -o <dir>
skyget get <URL> -o <dir> --sha256 <hex> --detach
skyget watch <job-id> --since 0
```

> skyget's version, daemon status and platform support follow its own release notes.  
> “skyget is available” does not mean “1Agent has a browser or network tool.”

## 4. 1A Bot

### What it is

1A Bot is the future persona/companion layer:

- Persona / SOUL / tone;
- Web / IM / Voice;
- Companion Contract;
- onboarding, privacy and data pages;
- “one voice” outward expression.

### Current status

**R&D, not released.**

- 1Agent does not depend on Bot;
- Bot does not own an authoritative copy of memory, skills or authority;
- Bot can only call 1Agent through the Surface API;
- without Bot, 1Agent remains a standalone Super-Agent;
- Bot does not change 1Agent's audit, authority or memory ownership.

## 5. Platform status

| Platform | Status | Notes |
|---|---|---|
| **Linux x86_64** | **Validated platform** | Release archive, installer, SHA256SUMS, provenance, TUI, CLI |
| **Windows x86_64** | **Internal build, not published** | Untested; no download; not a supported platform |
| **macOS** | **Planned** | After Linux stabilizes |
| **Android** | **Planned** | With the Bot/mobile companion |
| **iOS** | **Planned** | With the Bot/mobile companion |
| **Linux ARM64** | **Not released** | Separate build/validation needed |

Current release is **Linux-first**. Windows follows only after testing; macOS later; Android/iOS with Bot.

## 6. Release channels

- **GitHub Releases**: Linux archive, SHA256SUMS, Linux provenance; no Windows asset;
- **Source build**: Rust toolchain + `cargo build --release`;
- **Installer**: package `scripts/install.sh`;
- **Upgrade**: `1agent upgrade` / `1agent upgrade --rollback`;
- **Data migration**: `1agent export` / `restore` / `doctor`.

There is no auto-updater, system service installer or mobile installer yet.

## 7. Common misconceptions

- **Workshop is not required**; 1Agent still analyzes/plans without it.
- **skyget is not a browser tool**; it only transfers large files.
- **Bot is not a frontend shell**; it is the future persona/companion layer.
- **A Windows build existing does not mean Windows is downloadable or supported.**
- **There is no Android/iOS package today**; do not offer unofficial downloads.

---

[Back to README](../README.en.md) · [Features](features.en.md) · [Status](status.en.md)
