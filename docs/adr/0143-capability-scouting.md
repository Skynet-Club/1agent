# ADR 0143: Capability-First Tool Scouting

- Status: accepted for the P0 catalog/search slice; external discovery is planned
- Date: 2026-11-16
- Related: ADR-0142, `docs/capability-scouting-plan.md`, `docs/super-agent-doctrine.md`

## Context

The runtime exposes tools and a small skill registry, but a task such as “make a
product page” can still go straight to hand-written HTML. The agent knows tool
names and a model id, not its capability inventory: `1agent skill list` can be
empty while useful skill bundles live in other local roots, and there is no
search by capability. The result is both duplicated work and weak outcomes.

Task types are open-ended. A fixed enum of “web page / OCR / PDF / audio /
video” would be obsolete immediately. The durable unit is a capability:
`{inputs, operation, outputs, constraints}`.

## Decision

1. Add `capabilities/catalog.json`, a data file of capability cards. Each card
   carries keywords, inputs, operation, outputs, constraints, probe steps,
   candidate tools/skills/libraries/services/humans, verification recipes and
   fallbacks. Adding a capability is a catalog edit, not a new match arm.
2. Add `capability_search`, a read-only tool that searches the embedded catalog
   by capability phrase and returns ranked cards. It never installs or runs a
   candidate.
3. Seed five capabilities: UI/UX page design, OCR, PDF→HTML, audio
   transcription and video editing. The four non-design examples prove the
   schema is not web-specific.
4. Add the system-prompt rule: before a non-trivial build or conversion, search
   capabilities; prefer a trusted existing tool; check availability, license
   and security; if building, state why the best candidate was rejected; a tool
   running is not verification.
5. Keep P0 local and read-only. External discovery, availability checks, a
   `ToolChoice` receipt, adapters, composition and a tool-scout subagent are
   planned in `docs/capability-scouting-plan.md`.
6. Do not auto-install, auto-run or auto-trust a candidate. Trust levels are
   data (`core|verified|community|experimental`); only core/verified may be
   used automatically in later slices, and external tools require sandbox,
   license/security review and explicit approval.

## Consequences

- The agent can find a candidate class for OCR, PDF conversion, audio, video
  and design without hard-coding one product name.
- The catalog will go stale; P1/P2 add availability checks and external
  discovery.
- Search results are not evidence. The capability card's verification recipe
  remains mandatory.
- The `capability_search` tool adds one tool schema and one tool to the
  read-only surface; the tool count becomes 25.
- A future capability should be addable by editing JSON. If a capability needs
  a new execution primitive, an adapter is still code; the catalog alone does
  not execute anything.
- UI-UX-pro-max is one candidate, not the doctrine; if it is absent or
  unsuitable, the card lists alternatives and a build-from-scratch option.

## Evidence

```bash
cargo test --lib tools::capability
cargo test --lib system_prompt_requires_capability_search_before_building
cargo test --test tools_e2e
```

The tests cover catalog validation, deterministic ranking for the five
capabilities, tool listing and the prompt policy. They do not prove that an
external candidate is safe or that a novel task can be discovered; those are
P1/P2.
