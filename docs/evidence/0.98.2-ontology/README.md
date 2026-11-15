# 0.98.2 Ontology — Independent QA Evidence (Agent E)

Date: 2026-11-16 (local; final run captured 2026-11-15T18:42Z)
Owner: Agent E — independent QA
Scope: 0.98.2 read-only ontology projection (`ontology types|bindings|export|validate`)
Result: **commands green; amended P0 CQ gate met at 6 Yes + 5 named Partial
(11/16 at least partial); semantic P0 conditional acceptance**. The five No
CQs and the named Partial limitations remain explicit gaps, not green claims.

All fixtures are synthetic and live in temporary directories. This document
contains no production journal content, prompts, credentials, or secrets.

## 1. Tested tree

| Field | Value |
|---|---|
| Repository | `/home/skynet/1-agent` |
| Branch | `0.98-dev` |
| HEAD | `0c602946d42d907f0eed08b65c2127703abcb715` |
| HEAD subject | `docs(ontology): make the P0 CQ gate self-consistent and explicit` |
| Relevant commits | `471a16b` (semantic fixes), `f22f8e7` (this test suite), `102378f` (docs record), `0c60294` (CQ gate amendment) |
| Working tree | product tree **clean** at final run; only untracked evidence/.claude directories |
| Test file | `tests/ontology_cli.rs` (committed, 25 tests) |
| Evidence file | `docs/evidence/0.98.2-ontology/README.md` (untracked evidence) |

The exact bytes tested are identified by these SHA-256 values (captured
together with the command results):

```text
75a0cae06b0cd1e1d684f641a5ce8268d88c73dc30149f790b54f5f10dc8e2fc  src/ontology/mod.rs
aefbff5e200984a9ee77ef1d1a5d0886b2a38bc4093d2212f725cd85b4616329  src/ontology/export.rs
430b5a7194a1db561bd805b020a31fb3fb38da809986b6126a030d5d5066e31f  src/ontology/validate.rs
01b94104b54cf949da896109fe29f3355469b8b39af48965dfb07e60a2bb9e5c  src/ontology/types.rs
f3cc98ae2c06ea494a7a1df19d0ec6e4a43d838cd9520d042da687d9a0703cee  src/ontology/ids.rs
32633c8effc8676d1ba029ce81dc4cc4cba7710b7d73ee2744bf7b128f754f88  src/main.rs
ccef69c33ff4d46e1b4b0b01b9f82942bf7351b6c37298be79ec03fabf4055b0  src/tui_session.rs
c4f23cff347e5a4a203181fad71f73544b2125216ce42cfd9be66b6a305d7623  src/tools/mod.rs
75fae0261b2d38cb7a0734942f39a8c48c3509afca287a2abeda969b36b1a4d7  src/tools/files.rs
58de61ec6c77c019ad9f32ef60d2c643a32d1022be569924ed2f930fd35f1381  src/tools/subagent.rs
80aab7a437bdbc05fc2f496819ee9c23c29f747d9791627d9017e4b3c56218db  ontology/types.json
4518815d764d80f5a0ca749363ec9c2b49d3a83a419256f7b75f9f44d01047c3  ontology/bindings.json
8b1ed4b2e0ff47509787e5e8e501678890c0be31e71b546eab8b9e6dc5940d8c  tests/ontology_cli.rs
41cb8409c3c515386fcdaa6a0a9e7c7cda3ecfd118eb0d262c5db9013688cb05  docs/0.98.2-ontology-contracts.md
```

## 2. Commands and results

```bash
cargo test --offline --test ontology_cli
cargo test --offline --lib -- ontology
cargo check --offline
```

| Command | Result |
|---|---|
| `cargo test --offline --test ontology_cli` | **25 passed, 0 failed, 0 ignored** |
| `cargo test --offline --lib -- ontology` | **26 passed, 0 failed** (1055 filtered) |
| `cargo check --offline` | **success** (no warnings in the ontology change set) |

The coordinator separately reports that full `./scripts/verify.sh` passed at
the semantic/test commits; `0c60294` is docs-only, and QA re-ran the three
required commands above at `0c60294`.

The integration suite uses `env!("CARGO_BIN_EXE_first-principles-agent")` and
temporary directories. It drives the real CLI for export/validate behavior and
uses the public crate APIs only to synthesize journals, sessions, and tool
metadata.

Coverage in `tests/ontology_cli.rs`:

- `ontology types` / `ontology bindings`: non-empty, valid JSON, stable core IDs.
- Journal export: compact JSON-LD, embedded context/version/binding/registry
  hash, sorted `@graph`, deterministic bytes and SHA across two runs, stdout
  bytes equal `--out` bytes, validator accepts the export.
- Session export: Session/Run/Task/Goal links, v1 session compatibility,
  missing session exits non-zero and never creates `.sessions/<id>`.
- Validation: valid export exit 0; dangling `supportsClaim` and malformed /
  scalar JSON exit non-zero without panic.
- Privacy: raw objective, deliverable, mission, tool arguments, tool content,
  and a synthetic secret marker are absent; a prompt-derived session title is
  exported only as `titleSha256`/`titleChars`, never as raw title; allowed
  hashes/prose remain.
- Journal robustness: empty, whitespace-only, malformed-tail, `null`, and
  non-UTF-8 journals fail without panic and without partial stdout.
- Artifact lineage: `ToolObserved.metadata.sha256` → `Artifact` →
  `Observation` → `ToolInvocation` → `Run.used`, plus direct `write_file` /
  `edit_file` SHA metadata checks.
- CQ matrix below, including explicit negative tests for unanswerable CQs.

Four tests are explicitly **characterization tests of known gaps**
(`cq_gap_policy_*`, `cq_gap_eval_*`, `binding_coverage_*`, and
`validator_accepts_*_known_gap`). The CQ3/CQ6/CQ7/CQ9/CQ13 tests also pin
partial subfields (missing member/actor/local-ID/duplicate-side-effect/
failed-spawn details). They pass by asserting current behavior, not by proving
those CQs are complete. A green suite therefore means "current behavior is
pinned and the amended count gate is met", not "all CQs are fully answerable".

## 3. Findings

Severity: Blocker = acceptance cannot be claimed; High = contract/CQ failure;
Medium = correctness or privacy gap; Low = documentation/scope mismatch.

| # | Sev. | Finding | Evidence (file:line) | Test-confirmed |
|---|---|---|---|---|
| F1 | **Fixed (contract)** | The former §7 enumerated 11 CQs and §9 excluded CQ11/CQ15, making the literal "at least 10" unsatisfiable. `0c60294` amends §7 to require a published verdict for all CQ1–CQ16 and at least 10 answerable at Yes or named Partial, with every No/Partial listed in §9. The QA matrix has 6 Yes + 5 named Partial = 11, so the amended gate is met. | `docs/0.98.2-ontology-contracts.md:132-137`, `:172-175`; `docs/super-agent-ontology.md:115-130` | Yes — CQ tests below |
| F2 | **Fixed** | Forked sessions previously emitted `Session.forkedFrom` with no parent node, so `validate --session <fork>` failed. During QA the exporter was changed to inject a parent Session stub; `forkedFrom` now closes and validation exits 0. | `src/ontology/export.rs:71-82`; `src/ontology/validate.rs:61`, `:201-210` | Yes — `cq10_session_fork_is_exported_with_parent_stub_and_validates` |
| F3 | **Deferred (High impact)** | Tool policy/approval/action metadata is still not projected. `tools/mod.rs` writes `approval_card_id`, `policy_outcome`, and `policy_reason`, but the exporter only reads `sha256`/`path`/`relative_path`/`bytes` from observation metadata. CQ5, CQ12, CQ16 are unanswerable; CQ6 lacks actor/scope. The amended §7 explicitly excludes these action CQs from P0 and §9 defers R6 to 0.98.3, so this is an accepted capability gap, not an acceptance failure — but it must not be reported green. | `src/tools/mod.rs:372-378`; `src/ontology/export.rs:222-252`, `:552-598`; `docs/0.98.2-ontology-contracts.md:136-137`, `:168-169` | Yes — `cq_gap_policy_and_approval_metadata_is_not_exported`, `cq06_...` |
| F4 | **Fixed / Partial** | Successful spawn lineage is now exported: `SpawnAgentTool` metadata (`child_run_id`, `status`) produces a `SquadMember` with `parentRun`, `partOfSession`, `spawnedBy`, `childRunId`, and `Run.hasSquadMember`; CQ4 is answerable. Residual: failed/timed-out spawns return `ToolError` and have no child metadata, so they appear only as `Observation` nodes, not `SquadMember`; CQ13 remains partial. | `src/ontology/export.rs:254-290`, `:476-478`; `src/tools/subagent.rs:432-434`; `docs/0.98.2-ontology-contracts.md:157-162` | Yes — `cq04_successful_spawn_exports_squad_lineage` |
| F5 | **Fixed** | Evidence previously had no explicit observation edge. The exporter now derives `Evidence.observedBy` when `sourceRef` is `tool:<call-id>` and a matching Observation exists; CQ2 has an explicit chain. | `src/ontology/export.rs:371-380` | Yes — `cq02_claim_evidence_observation_join_uses_explicit_observed_by` |
| F6 | Medium | The validator does not detect a completed claim without evidence, although plan R4/F4 requires it. A completed Answer with a claim that has no `cites` validates `ok=true`. The frozen contract §6 does not list this check, so contract and plan disagree. | `src/ontology/validate.rs:235-245`; `docs/0.98.2-development-plan.md:124`, `:227` | Yes — `validator_accepts_a_completed_claim_without_evidence_known_gap` |
| F7 | **Fixed / Partial** | `ResumeStarted.original_run_id` was previously ignored. The exporter now emits `Run.resumedFromRunId` alongside `resumedFromCheckpointSeq`, so the originating run and checkpoint are identifiable. Duplicate side effects have no first-class field and remain a consumer-side/gap item (arguments are not exported). | `src/ontology/export.rs:144-155`, `:504-508` | Yes — `cq09_resumed_run_reports_original_run_and_checkpoint` |
| F8 | **Fixed** | Prompt-derived session titles were exported verbatim. The exporter now emits `titleSha256`/`titleChars` instead of `title`; the raw prompt prefix is absent from JSON-LD and CLI output, and a regression test covers it. | `src/tui_session.rs:778-791`; `src/main.rs:15801`, `:16482`, `:21779`; `src/ontology/export.rs:51-58`; `docs/0.98.2-ontology-contracts.md:148-150` | Yes — `prompt_derived_session_title_is_hashed_and_redacted` |
| F9 | Medium | Bindings are illustrative, not validated. The registry has 17 types and 11 bindings; 9 types have no binding and no explicit `Unknown`: Task, Observation, Answer, Goal, Commitment, ActionType, PolicyDecision, Approval, DataBinding. Binding entries lack `mappingVersion`/`owner`/`transform`, source-path existence is not checked, and `ToolInvocation.executedBy` points at `ToolCalled.context`, which does not exist in `ToolCall`. Plan F2 requires 100% core coverage or explicit Unknown and validator failure on drift. | `ontology/bindings.json:30-35`; `src/domain.rs:975-979`; `src/ontology/types.rs:95-100`, `:134-176`; `docs/0.98.2-development-plan.md:122`, `:179-185` | Partly — `binding_coverage_gap_is_documented` |
| F10 | Low | An empty journal inside a session is silently skipped (`build_run` returns `None`), while standalone empty journal export fails. Plan F4 asks for a known-exception list, not silent omission. | `src/ontology/export.rs:43`, `:96-98`, `:78-85`; `docs/0.98.2-development-plan.md:229` | No |
| F11 | Low | Plan F3 documents `--format ttl`; the CLI accepts only `jsonld` and rejects everything else. P0 contract only requires JSON-LD, so this is a plan/CLI scope mismatch, not a P0 code failure. | `src/main.rs:2690-2691`, `:7872-7874`; `docs/0.98.2-development-plan.md:193` | No |

### Fixes needed (product code not edited by QA)

Resolved by `0c60294`: the CQ gate is now self-consistent and the current
6 Yes + 5 named Partial matrix meets it. Remaining items:

1. §9 records the full No/Partial matrix, but per-CQ owner/version is explicit
   only for the grouped R6 deferral and CQ13/CQ11/CQ15; spell out owner/version
   for the CQ3/CQ6/CQ7/CQ9 partials to satisfy §7 literally.
2. Policy/approval/action: project `policy_outcome`, `approval_card_id`,
   authority/actor/scope, or record an explicit owner-accepted 0.98.3 exclusion
   for CQ5/CQ6-actor/CQ12/CQ16.
3. CQ13 failure path: export failed/timed-out spawns as SquadMember nodes with
   an error status, or narrow CQ13 to successful members.
4. CQ3: add an Artifact-to-producing-member link if "member" is required, or
   narrow CQ3 to run/tool-call producer plus SHA.
5. CQ7: publish a target-ID convention or emit FlipCondition/Unknown nodes if
   consumers need resolvable IRIs rather than answer-local IDs.
6. CQ9: export a duplicate-side-effect indicator, or narrow CQ9 to
   resume lineage (which is now fully answerable).
7. Validator: implement the plan R4/F4 claim-without-evidence check, or remove
   it from the plan and record the exception.
8. Bindings: add source-path/type/required/mappingVersion validation and a
   coverage report, or narrow R2/G3 and record the explicit `Unknown` list.
9. Fix the plan/CLI `ttl` mismatch and the empty-journal silent skip, or record
   both as accepted limitations.

## 4. CQ matrix (CQ1–CQ16)

Verdict definitions:
- **Yes** = answerable from the exported JSON-LD without external conventions.
- **Partial** = some fields exist, but a required part of the question is
  missing, local-only, or only recoverable by an unpublished convention.
- **No** = no P0 export data.

| CQ | Question (short) | Verdict | Available in P0 / missing | Test |
|---|---|---|---|---|
| CQ1 | Which claims in a `completed` answer have no evidence? | **Yes** | `Answer.status`, `Claim.cites` absence | `cq01_...` |
| CQ2 | Which evidence supports claim X; which tool observation? | **Yes** | `Claim.cites`, `Evidence.observedBy` → Observation → ToolInvocation | `cq02_...` |
| CQ3 | Which run/member/tool call produced artifact A; SHA? | **Partial** | Artifact SHA → Observation → ToolInvocation → `Run.used`; no Artifact-to-producing-member link | `artifact_sha_metadata_joins_...` |
| CQ4 | Which parent run/session/`spawn_agent` call started a member? | **Yes** | Successful spawn exports SquadMember (`parentRun`, `partOfSession`, `spawnedBy`, `childRunId`) and `Run.hasSquadMember` | `cq04_...` |
| CQ5 | Which authority profile authorized a tool; was it approved? | **No** | Policy/approval metadata dropped | `cq_gap_policy_...` |
| CQ6 | Which tasks blocked on `needs_authorization`; which actor/scope? | **Partial** | `Run.hasOutcome=needs_authorization`, Task link; no actor/scope | `cq06_...` |
| CQ7 | Which answers are conditional; which claims/unknowns do flips target? | **Partial** | `Answer.status=conditional`, `flipConditions.targetIds`; targets are answer-local IDs (matched to `hasClaim` suffix / embedded `unknowns`), no FlipCondition/Unknown IRIs | `cq07_...` |
| CQ8 | Which evidence is expired or has unknown observation time? | **Yes** | `expiresAtUnixMs`, optional `observedAtUnixMs` | `cq08_...` |
| CQ9 | Which run resumed from which checkpoint; duplicate side effects? | **Partial** | `resumedFromRunId`, `resumedFromCheckpointSeq`, `checkpointSeq`; no duplicate-side-effect field | `cq09_...` |
| CQ10 | Which session forked from which parent? | **Yes** | `Session.forkedFrom` plus injected parent Session stub; `validate --session <fork>` exits 0 | `cq10_...` |
| CQ11 | How do eval cases/runs map to claims/runs? | **No** | No eval projection | `cq_gap_eval_and_audit_...` |
| CQ12 | Which artifact SHA changed after approval? | **No** | No Approval/Policy; no SHA history per artifact | `cq_gap_policy_...` |
| CQ13 | Which members failed/timed out; how did the parent handle it? | **Partial** | Failed/timed-out spawns appear as Observation with status; no SquadMember/child Run link for them | `cq04_...` (CQ13 assertions) |
| CQ14 | Which provider/model/route produced a claim; cost? | **Yes** | Claim IRI contains run; `Run.providerId/model/routeReason/inputTokens/outputTokens/costMicros` | `cq14_...` |
| CQ15 | Which user-visible conclusions lack audit/replay-passed journals? | **No** | No Audit/Journal replay projection | `cq_gap_eval_and_audit_...` |
| CQ16 | Which irreversible actions were approved by whom and when consumed? | **No** | No ActionCard/Approval/ActionLog; policy metadata dropped | `cq_gap_policy_...` |

Summary: **6 Yes, 5 Partial, 5 No**. Yes: CQ1, CQ2, CQ4, CQ8, CQ10, CQ14.
Partial: CQ3, CQ6, CQ7, CQ9, CQ13. No: CQ5, CQ11, CQ12, CQ15, CQ16. Counting
Yes or named Partial as answerable gives **11/16**, which meets the amended §7
gate ("at least 10 at Yes or named Partial") introduced in `0c60294`. This is
not a claim that the five Partial or five No CQs are complete; their missing
fields/edges are listed above and must remain visible. §9 itself records this
exact 6/5/5 matrix at `docs/0.98.2-ontology-contracts.md:172-175`.

## 5. Acceptance decision

**Amended P0 CQ gate: met. Semantic P0 projection: conditionally acceptable.**

Evidence for the verdict:

- All three required commands pass at `0c60294`: 25/25 integration tests,
  26/26 ontology lib tests, and full `cargo check --offline`.
- Determinism, compact JSON-LD bytes/SHA, embedded registry metadata, privacy
  redaction, v1/malformed-journal robustness, artifact SHA lineage, and
  write/edit metadata are covered by passing tests.
- The P0 semantic gaps raised during QA are fixed: CQ2 `observedBy`, CQ9
  `resumedFromRunId` lineage, CQ10 fork parent stub with successful validation,
  CQ4 successful-spawn SquadMember, and title hashing.
- `0c60294` amends §7 so that "at least 10 answerable" means Yes or named
  Partial. The QA matrix is **6 Yes / 5 Partial / 5 No = 11/16 answerable at
  Yes or named Partial**, and §9 records the same matrix. The amended gate is
  therefore met.

Remaining explicit gaps that must stay visible (they are not green claims):

- **No:** CQ5, CQ11, CQ12, CQ15, CQ16 — policy/approval/action, eval, and
  audit/replay projections. §9 defers R6 action/policy/approval to 0.98.3 and
  states CQ11/CQ15 are not P0-answerable.
- **Partial:** CQ3 (no Artifact-to-producing-member link), CQ6 (actor/scope
  absent), CQ7 (answer-local target IDs, no FlipCondition/Unknown IRIs), CQ9
  (no duplicate-side-effect indicator), CQ13 (failed/timed-out spawns lack
  SquadMember nodes).
- **Code/test gaps:** the validator accepts a completed claim without
  evidence; bindings lack full coverage/source-path validation; empty session
  journals are silently skipped; and the plan/CLI `ttl` scope mismatch remains.
- **Documentation note:** §9 lists the full No/Partial matrix, but per-CQ
  owner/version is explicit only for grouped items (R6, CQ13, CQ11/CQ15);
  spelling it out for CQ3/CQ6/CQ7/CQ9 would satisfy §7 literally.

Conditional acceptance means the semantic P0 projection is acceptable for
0.98.2 provided the No/Partial list above remains explicit in release notes and
is not reported as complete. Do not claim full CQ coverage or "all gaps fixed".
