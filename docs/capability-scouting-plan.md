# Capability-First Tool Scouting — Development Plan

Status: P0 implemented; P1–P4 planned
Date: 2026-11-16
Related: [ADR-0143](adr/0143-capability-scouting.md), [ADR-0142](adr/0142-loopback-preview.md), [Super-Agent doctrine](super-agent-doctrine.md)

## 0. One sentence

The task types are open-ended; the capability grammar is finite. A Super-Agent must decompose a task into capabilities, find the most fitting tool for each capability, compose them, and verify the outcome.

## 1. Problem

The current runtime exposes tools and a small skill registry, but it has no capability-first discovery layer:

- A task such as “make a product page” goes straight to hand-written HTML even when a design skill/design system exists.
- `1agent skill list` can be empty while useful skill bundles live in other local roots.
- Tool names are hard-coded in prompts; the agent does not search by capability.
- The agent can verify a file hash or an HTTP 200, but not whether the artifact meets its purpose.
- Future tasks (OCR, PDF conversion, audio transcription, video editing, data pipelines, research, operations) cannot be handled by adding one prompt branch per task type.

The four examples in the discussion are regression tests, not the product boundary.

## 2. Principles

1. **Task → capability graph, not task-type enum.** Decompose until each leaf is an executable/verifiable capability.
2. **Capability-first search.** Search by `{input, operation, output, constraints}`, not by a remembered product name.
3. **Discover → evaluate → adapt → verify → record.** Finding a tool is not enough; integration and outcome verification are part of the capability.
4. **“Handy” means total fit.** Fit, integration cost, license/security, maintenance, data boundary, reversibility and observability.
5. **Local first.** Prefer already-installed tools, project dependencies and the system toolchain before adding a dependency.
6. **No tool worship.** A 20-line script can beat a framework; a human/GUI can beat a CLI. “知止” brakes “尚贤”.
7. **Open-world, but governed.** Unknown tools are untrusted: sandbox, pin, license/security review and explicit approval for installs/cloud/irreversible actions.
8. **Verification is capability-specific.** OCR uses sample error/reading order; PDF uses layout screenshots; audio uses WER/timestamps; video uses duration/codec/frames.
9. **Record the choice.** Candidates, chosen tool, rejected reasons, verification and fallback go into the journal/answer.
10. **Learn into a playbook.** Capability → tool → context → outcome → cost becomes reusable evidence, revalidated on version changes.

## 3. Architecture

```text
User task
  → goal, constraints, deliverables
  → recursive capability graph
  → per node: local inventory → external candidates → score → adapt → verify
  → compose workflow (data contracts, fallbacks)
  → outcome verification + journal receipt
  → playbook update
```

### 3.1 Capability cards

A capability card is data, not a Rust enum:

- id, name, summary, keywords;
- inputs, operation, outputs, constraints;
- probe steps (for example `pdffonts`, `ffprobe`);
- candidate tools with kind/source/license/trust/notes;
- verification recipe;
- fallback.

Adding a future capability should be a catalog edit, not a new match arm.

### 3.2 Tool scout

A bounded search phase or subagent:

1. Restate the capability without naming a tool.
2. Probe the input and environment.
3. Search local registry, installed CLIs, project dependencies, external registries and known skill roots.
4. Return 2–3 candidates with metadata; the main agent decides.
5. Stop when a candidate is good enough; do not search endlessly.

### 3.3 Adapters and composition

Tools have different interfaces. Each candidate needs a thin adapter contract: typed input, typed output, permissions, timeout, artifacts, error mapping. Composition needs data contracts and fallbacks between nodes.

### 3.4 Verification and trust

Each capability carries a verification recipe. Tool output is not evidence until the recipe passes. Trust levels: `core`, `verified`, `community`, `experimental`; only the first two may be used automatically, and external tools are sandboxed/pinned.

## 4. Phases

### P0 — Capability catalog + search (this slice)

- Add `capabilities/catalog.json` with six initial capability cards: UI/UX design, UI review/unslop, OCR, PDF→HTML, audio transcription and video editing.
- Add `capability_search`, a read-only tool that searches the embedded catalog by capability phrase and returns candidates, probe steps, verification recipes and fallbacks.
- Add the system-prompt rule: search capabilities before a non-trivial build; prefer a trusted existing tool; if building, state why the best candidate was rejected; a tool running is not verification.
- Add regression tests for search ranking, catalog validation, prompt policy and tools listing.
- Document the plan, ADR and tool surface.

Acceptance:

- `1agent tools` lists `capability_search`.
- The search returns OCR for “扫描 PDF 提取文字”, video for “视频剪辑”, audio for “音频转文字”, PDF for “PDF 转 HTML”, design for “产品落地页设计”, and UI review for “unslop 页面 AI 味 验收”.
- Full `./scripts/verify.sh` passes.
- No auto-install, no network, no side effects.

### P1 — Local inventory and choice receipt

- Check candidate availability: PATH binaries, registered tools, local skill roots.
- Add a durable `ToolChoice` record (candidates, chosen, rejected reasons, verification, fallback) to the journal/answer.
- Add a CLI `1agent capability search` for manual inspection.
- Register/adapt the first design skill and a media tool candidate.

### P2 — Open-world discovery

- External indexes: package registries, GitHub/docs, model hubs, cloud APIs, MCP.
- Tool-scout subagent with a bounded budget and provenance.
- License/security/maintenance scoring; approval gates for installs and cloud calls.

### P3 — Adapters, composition and verification

- Generic adapter contract for CLI/library/service/skill/human.
- Capability graph execution with data contracts, retries/fallbacks and artifact store.
- Verification recipes for OCR/PDF/audio/video/design.
- Ontology nodes for Capability, Tool, ToolChoice and Verification.

### P4 — Evaluation and learning

- Held-out novel tasks (not in the catalog) to test decomposition and discovery.
- Process metrics: capability coverage, candidate quality, build-vs-buy justification, integration time, total cost, rework, verification strength.
- Playbook updates from completed tasks; periodic revalidation of pinned tools.

## 5. P0 initial capability cards

| Capability | Inputs | Output | Probe | Verification |
|---|---|---|---|---|
| UI/UX design | brief/constraints | page/design system | product/company, audience, CTA, brand assets/copy, existing stack | 390/768/1440 screenshots, no horizontal scroll, focus/contrast, real CTA |
| UI review / unslop | HTML/screenshot/brief | revised page + audit | read the brief, find AI-slop tells, check first screen and CTA | before/after screenshots, copy 5-second test, slop checklist cleared |
| OCR | image/scanned PDF | text/JSON/searchable PDF | `pdffonts`, `pdftotext`, language | sample CER, confidence, reading order |
| PDF→HTML | PDF | HTML + assets | text layer, columns/tables | text fidelity, layout screenshot, links/images |
| Audio→text | audio | text/timestamps | `ffprobe`, language, speakers | sample WER, timestamps, diarization |
| Video editing | video + instructions | video | `ffprobe` codec/fps/duration | duration, codec, frames, subtitles, A/V sync |

Candidates are intentionally classes of tools, not a single product; the catalog can be extended or replaced without changing the search tool.

## 6. Non-goals for P0

- No automatic installation or execution of external tools.
- No universal marketplace.
- No claim that six cards cover future tasks.
- No automatic visual judgment; design verification remains a separate gate (P3).
- No change to Authority/Policy; capability search is read-only.

## 7. Risks

- **Stale catalog:** capability cards age; P1/P2 add availability checks and external discovery.
- **Prompt overuse:** the model may search for trivial tasks; the prompt limits it to non-trivial builds/conversions.
- **Tool trust:** open-source tools can be unmaintained or malicious; trust levels, license review and sandboxing are mandatory.
- **Over-tooling:** a small script may be the right answer; “知止” and build-vs-buy justification remain part of the decision.
- **Verification gap:** search results are not evidence; the verification recipe must run.

## 8. Verification

P0 evidence:

```bash
cargo test --lib capability
cargo test --lib system_prompt_requires_capability_search
cargo test --test tools_e2e
./scripts/verify.sh
```

The tests prove catalog validation, deterministic ranking, the read-only tool contract and the prompt policy. They do not prove that a future external tool is safe or that a novel capability is discoverable; those are P1/P2.
