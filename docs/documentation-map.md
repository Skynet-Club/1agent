# Documentation Map

Date: 2026-11-12

## 1A Bot development manual (Obsidian)

The manual lives in the Obsidian vault at
`Skynet-Club/III/90-参考/1A-Bot/` (13 chapters + ADR, index: `00-索引`). It is the single source of
truth for the Bot Alpha program; the repository no longer keeps a duplicate. The existing V0
boundary remains until ADR-003 is accepted.

## Stable technical references

- [`architecture.md`](architecture.md): runtime layers, security behavior and limitations.
- [`threat-model.md`](threat-model.md): P0/P1 threat inventory, trust boundaries and open gaps.
- [`rfc/0001-module-boundaries.md`](rfc/0001-module-boundaries.md): session/harness/sandbox
  interfaces and the `main.rs`/`tui.rs` split plan.
- [`io-contracts.md`](io-contracts.md): task, model, tool, answer, journal and report contracts.
- [`model-routing.md`](model-routing.md): hard filters, objectives, fallback and budgets.
- [`evaluation.md`](evaluation.md): suite kinds, metrics, gates and invalid claims.
- [`self-evolution.md`](self-evolution.md): L2 lifecycle, evidence and promotion rules.
- [`product-boundary.md`](product-boundary.md): supported outcomes, non-goals and claim limits.
- [`tui.md`](tui.md): terminal interaction contract, safety and current limitations.
- [`0.98-session-interaction.md`](0.98-session-interaction.md): session picker,
  follow-up context/queue, interrupt/resume and Super-Agent interaction boundaries.
- [`super-agent-ontology.md`](super-agent-ontology.md): 1AO ontology, competency
  questions, current-data mapping, migration phases and trade-off analysis;
  machine-readable drafts in [`../ontology`](../ontology).
- [`0.98.2-development-plan.md`](0.98.2-development-plan.md): Semantic Lineage
  release plan (requirements → features → tasks → gates → evidence).
- [`0.98.2-plan-review.md`](0.98.2-plan-review.md): plan strengths/weaknesses,
  scope options, kill criteria and solutions.
- [`0.98.2-ontology-contracts.md`](0.98.2-ontology-contracts.md): frozen P0
  export/ID/validation contract and parallel file ownership.
- [`0.99-goal-kernel-plan.md`](0.99-goal-kernel-plan.md): GoalKernel,
  ActionRuntime and DelegationKernel plan after 0.98.2 acceptance.
- [`0.99-handoff.md`](0.99-handoff.md): resume checkpoint — what is accepted,
  what remains, branches/tags, evidence paths and the first next task.
- [`master-agent.md`](master-agent.md): Master-Agent contracts, commands, safety and live evidence.
- [`master-multi-goal-production-line.md`](master-multi-goal-production-line.md): planned
  multi-Goal, multi-Master intake, supervision, dispatch and verification program.
- [`master-capability-matrix-v1.json`](master-capability-matrix-v1.json): bounded runtime probe evidence.
- [`master-live-acceptance-v1.json`](master-live-acceptance-v1.json): machine-readable live acceptance.
- [`status-v0.97.md`](status-v0.97.md): 0.97 metric review and release blockers.
- [`rollback-v0.97.md`](rollback-v0.97.md): binary/journal/config rollback, kill switch,
  incident procedure and known limitations.
- [`status-v0.93.md`](status-v0.93.md): implementation status at 0.93.
- [`audit-v0.93.md`](audit-v0.93.md): requirement-to-evidence audit.
- [`handoff-v0.93.md`](handoff-v0.93.md): current resume and verification guide.
- [`next-phase-evidence.md`](next-phase-evidence.md): real-provider and target-host evidence plan.
- [`handoff-v0.92.md`](handoff-v0.92.md): current development state and resume instructions.
- [`research-foundations.md`](research-foundations.md): papers, engineering sources and adoption.
- [`development-flywheel.md`](development-flywheel.md): required iteration sequence.

## Product

- [`product-manual.md`](product-manual.md): the product manual — proposition, drill story,
  five behavior rules, interaction samples, mechanisms, evidence boundaries, terminology.
- [`super-agent-doctrine.md`](super-agent-doctrine.md): Super-Agent judgment doctrine —
  four silent questions, the four schools as lenses, precedence, 12 judgment scenarios and
  the current prompt/test boundary.
- [`capability-scouting-plan.md`](capability-scouting-plan.md): capability-first tool scouting —
  capability cards, search, trust, adapters, verification and the P0–P4 development plan.
- [`acceptance-gate-plan.md`](acceptance-gate-plan.md): completion acceptance —
  contract, independent verifier, receipt, decisions and the P0–P3 gate plan.
- [`claims.md`](claims.md): 说法与证据对照表 — every external claim paired with a rerunnable
  evidence command, or explicitly marked as unproven.
- [`evidence-v0.96.md`](evidence-v0.96.md): first live-provider pilot on the reframing
  corpus, including what it does and does not license.
- [`evidence-heldout-v1.md`](evidence-heldout-v1.md): first frozen held-out run; precision
  below the pre-registered threshold, with the failing cases identified.
- [`evidence-heldout-v2.md`](evidence-heldout-v2.md): second frozen held-out run; precision
  above the threshold, with the remaining failure class and caveats stated.
- [`output-contract.md`](output-contract.md): the Occam contract for answers — necessity test,
  the four items that must survive brevity, forbidden patterns and four metrics.
- [`participation.md`](participation.md): how outside people can contribute — labelling,
  real requests, blind comparison, red team — with the rules that keep the data usable.
- [`evidence-answer-style-v0.md`](evidence-answer-style-v0.md): pre-change baseline for answer
  length, restatement and menus, with the corpus limitations stated.
- [`evidence-answer-style-v1.md`](evidence-answer-style-v1.md): paired replay of the same 14
  tasks; brevity improved, menus and language regressed.
- [`evidence-reframe-live-v0.md`](evidence-reframe-live-v0.md): prompt unified into one file,
  plus one live reframe call; smoke only.
- [`evidence-answer-style-v2.md`](evidence-answer-style-v2.md): the same 14 tasks after the
  language clause; length improved, wrong-language answers fixed.
- [`evidence-answer-style-v3.md`](evidence-answer-style-v3.md): a fresh 14-task style set with
  pre-registered targets; all five targets met on one run.
- [`evidence-answer-style-v2set.md`](evidence-answer-style-v2set.md): a second, unrelated
  14-task style set; the same five targets hold.
- [`evidence-annotation-round1.md`](evidence-annotation-round1.md): first external labelling
  round; one annotator, so no kappa yet, with the single disagreement recorded.
- [`evidence-annotation-round2.md`](evidence-annotation-round2.md): second annotator; kappa
  0.891, and both annotators dispute the label the only held-out error depends on.

## Thesis and pre-registration (v0.95 research)

- [`thesis/00-problem.md`](thesis/00-problem.md): the product proposition, the five-layer
  vocabulary, the stop rule, the three invariants and the falsification conditions.
- [`thesis/01-need-graph.md`](thesis/01-need-graph.md): NeedGraph contract, machine-checkable
  invariants and additive composition with the existing protocol.
- [`thesis/02-eval-protocol.md`](thesis/02-eval-protocol.md): pre-registered evaluation
  protocol, labeling rules, A/B baseline, metrics and kill criteria.
- [`thesis/FREEZE.txt`](thesis/FREEZE.txt): SHA-256 freeze of the above.

## Architecture decisions

- [`adr/0001-rust-runtime.md`](adr/0001-rust-runtime.md)
- [`adr/0002-jsonl-journal.md`](adr/0002-jsonl-journal.md)
- [`adr/0003-openai-compatible-provider.md`](adr/0003-openai-compatible-provider.md)
- [`adr/0004-evaluation-report-trust.md`](adr/0004-evaluation-report-trust.md)
- [`adr/0005-terminal-ui-presentation-adapter.md`](adr/0005-terminal-ui-presentation-adapter.md)
- [`adr/0006-raw-terminal-presentation.md`](adr/0006-raw-terminal-presentation.md)
- [`adr/0007-ratatui-interaction-layer.md`](adr/0007-ratatui-interaction-layer.md)
- [`adr/0008-tui-command-scopes-and-session-events.md`](adr/0008-tui-command-scopes-and-session-events.md)
- [`adr/0009-explicit-command-modes-and-evidence-views.md`](adr/0009-explicit-command-modes-and-evidence-views.md)
- [`adr/0010-session-bound-master-plan-approval.md`](adr/0010-session-bound-master-plan-approval.md)
- [`adr/0011-session-to-master-dispatch.md`](adr/0011-session-to-master-dispatch.md)
- [`adr/0012-master-supervision-and-human-response.md`](adr/0012-master-supervision-and-human-response.md)
- [`adr/0013-explicit-artifact-verification-views.md`](adr/0013-explicit-artifact-verification-views.md)
- [`adr/0014-token-headroom-session-workspace-presets.md`](adr/0014-token-headroom-session-workspace-presets.md)
- [`adr/0015-provider-usage-ledger-and-threshold-compaction.md`](adr/0015-provider-usage-ledger-and-threshold-compaction.md)
- [`adr/0016-unified-capability-probe-evidence.md`](adr/0016-unified-capability-probe-evidence.md)
- [`adr/0017-goal-key-result-task-list.md`](adr/0017-goal-key-result-task-list.md)
- [`adr/0018-deterministic-global-scheduler.md`](adr/0018-deterministic-global-scheduler.md)
- [`adr/0019-requirement-regression.md`](adr/0019-requirement-regression.md)
- [`adr/0020-reframe-journal-events.md`](adr/0020-reframe-journal-events.md)
- [`adr/0139-read-only-ontology-projection.md`](adr/0139-read-only-ontology-projection.md):
  read-only 1AO JSON-LD projection, deterministic export and validation boundary.

## Current audits and release evidence

- [`audit-v0.94.md`](audit-v0.94.md): current cross-requirement audit.
- [`status-v0.94.md`](status-v0.94.md): current implementation evidence.
- Historical `status-v0.*.md` files preserve incremental implementation evidence.
