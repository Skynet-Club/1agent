# ADR 0145: Understanding Gate, Text-Required Clarify and Design Delivery Guard

- Status: accepted
- Date: 2026-11-16
- Related: ADR-0144, `docs/acceptance-gate-plan.md`, `src/clarify.rs`, `src/runtime.rs`

## Context

A real run of "给我做个网站" asked four clarify questions. The user chose
`我这就给你名字和几句介绍` (I will give you the name and copy) on the content
question. That option was a plain choice, so the panel recorded it as answered
without collecting any text. The model then built a page made entirely of
`【产品名】` placeholders, and `acceptance_gate` accepted a contract whose own
objective said "占位文案". Missing screenshots were only a medium residual risk.

Three failures compounded:

1. The clarify menu let the user express intent without providing content.
2. The model treated "I will provide" as "provided" and started building.
3. The acceptance contract was allowed to define placeholder content as the
   deliverable, so the gate verified the wrong contract.

## Decision

1. Add `ClarifyOption.requires_text`. Selecting such an option opens the text
   field; empty text is not an answer. `Other` keeps the same rule.
2. Normalize a promise-to-provide label (`我这就给`, `i will provide`, etc.) to
   `requires_text` as a backstop. The promise alone cannot become an answered
   question.
3. Include `other_text` in `ClarifyAnswer` and validate it in `resolve_round` and
   `ClarifyPanel::select`. `blocking_unanswered > 0` means the brief is not
   complete; the prompt tells the model to ask again or stop, not build.
4. Add an understanding rule to the system prompt: for a non-trivial build,
   state the goal, user, deliverable, critical inputs, constraints and
   acceptance test first. Missing input means ask and wait. Never invent a
   product, brand, customer, quote, number, screenshot or result.
5. Add a page delivery guard to the runtime completion gate:
   - every written `.html`/`.htm` artifact is scanned for placeholder markers
     (`【`, `占位`, `待填`, `待替换`, `TODO`, `FIXME`, `lorem ipsum`,
     `example.com`, `Your Company`, unreadable files);
   - a page acceptance contract must name 390/768/1440 viewport evidence,
     horizontal-scroll checks, placeholder checks, mobile tap targets, brief
     completeness, a real CTA and design review evidence;
   - a completed answer that still contains placeholders, or whose accepted
     receipt omits those checks, is downgraded to `conditional` and journaled
     as `CompletionArtifactUnfinished` or `CompletionDesignContractGap`.
6. Keep the design capability card's verification recipe in sync with the
   runtime guard.
7. Let `browser` open loopback preview pages without `--allow-network` when
   `allow_loopback=true`, and add `action=audit`. The audit captures real
   390/768/1440 layout metrics (scrollWidth/clientWidth, placeholder hits,
   tap-target sizes, h1 count) and PNGs, so a local page can be verified without
   a terminal session or external network.
8. Show the answer status, not only the process status, in the TUI:
   `completed` remains “已完成”; `conditional` is “有条件完成”;
   `needs_evidence`/`needs_authorization` are “需要处理”. A conditional
   delivery must not look like a clean success.
9. Reuse a model-authored “Other, I will explain” option instead of adding a
   second automatic Other; it collects text and is the only Other shown.
10. Make browser approval cards survive a later human turn. The card no longer
    matches on the runtime `action_id` (which necessarily changes between
    runs); it still matches action, target, browser session, origin and
    optional artifact digest, is one-shot, and lasts 10 minutes. Opening the
    same loopback page again is idempotent and does not reload it, preserving
    the DOM-bound card. The prompt tells the model not to reopen/navigate after
    `approval_required`.

## Consequences

- A promise to provide content can no longer be silently converted into an
  answered brief.
- Placeholder pages can no longer be `completed` merely because the model wrote
  a contract that accepts placeholders.
- The guard checks the artifact and the contract, not the model's prose.
- It does not prove that supplied screenshots are real, that the brief is true,
  or that the verifier is independent; those remain P2. The guard is a
  fail-closed floor, not a substitute for a design reviewer.
- HTML written through `terminal_exec` is not scanned; `write_file`/`edit_file`
  metadata is the supported path.

## Evidence

```bash
cargo test --lib clarify
cargo test --lib page_with_placeholders
cargo test --lib page_contract_without_viewport
cargo test --lib clean_page_with_complete_contract
cargo test --lib system_prompt_requires_understanding_before_building
./scripts/verify.sh
```

The tests cover text-required options, promise normalization, empty text,
blocking answers, placeholder artifacts, incomplete contracts and the clean
page path.
