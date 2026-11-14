# 1Agent 0.97 Independent Code-Level Audit

Status: **independent code-level review completed; not a third-party certification**
Snapshot: `03ad40a` reviewed, fixes landed in `74be874` and `1ac7b5a` (2026-11-14)
Scope: local runtime, permissions, tools/MCP, provider egress, journal/replay,
packaging and release scripts
Out of scope: external IdP, multi-tenant deployment, macOS/Windows runtime,
formal compliance mapping, and a human blind review

## 1. Method

Four reviewers with fresh, read-only contexts reviewed the tree independently:

1. permission, authority, approval and execution safety;
2. network egress, secret handling and prompt injection;
3. journal, replay, audit, metrics and side-effect accounting;
4. release packaging, reproducibility, provenance and supply chain.

They read code and ran offline tests; they did not edit files, call providers or
perform network requests. Every finding below was re-checked by the owner of
this document before being marked fixed or accepted.

Result: **no P0 finding**. Five P1 classes were fixed; four P1/P2 areas remain
open and are listed in section 4. This is stronger than a self-review but it is
not an external audit attestation and must not be described as one.

## 2. Fixed findings

| # | Severity | Finding | Fix | Regression test |
|---|---|---|---|---|
| 1 | P1 | `ExecutionGate` returned `RequireApproval` but the runtime only rejected `Err`, so write/execute tools ran without approval | `execute_tool` now accepts only `PolicyOutcome::Allow`; every other outcome returns `approval_required` | `runtime::tests::execution_gate_denies_tools_that_require_approval` |
| 2 | P1 | `spawn_agent` was classified as a read tool; a ReadOnly Authority profile let the model ask for a write-enabled child, and the child did not inherit the Authority gate | `spawn_agent` is now write-capable, so read-only/KILL/write-level Authority gates run before any child starts | `tools::subagent::tests::spawn_is_write_capable_so_read_only_gates_cannot_bypass_it` |
| 3 | P1 | MCP stdio servers inherited the full agent environment, including provider keys, webhook tokens and token-exchange secrets | `env_clear()` plus a minimal allowlist (`PATH`, `HOME`, `TMPDIR`, locale) and the operator's `server.env` | `tools::mcp` tests (stdio discovery/call) |
| 4 | P1 | MCP tools trusted server annotations (`readOnlyHint`) and ignored `PermissionSet`, so a malicious server could bypass `--read-only`, KILL and Authority checks and make network calls without `--allow-network` | every MCP tool is write-capable; `execute` requires `allow_network`; server hints cannot lower local permission | `tools::mcp` tests updated |
| 5 | P1 | Resume accepted a journal whose last checkpoint was followed by a paired tool call; the model could repeat an already-completed write with a new call id | `find_resumable_prefix` rejects a tool call after the last checkpoint with `UncheckpointedToolCall` | `journal::tests::resume_rejects_a_tool_call_after_the_last_checkpoint` |
| 6 | P2 | Provider HTTP client followed redirects; reqwest may keep `Authorization` on a same-host scheme downgrade | provider client now uses `redirect::Policy::none()` | provider/config tests |
| 7 | P2 | Packages could be built from a dirty tree or a non-tag commit; `install.sh` did not verify `SHA256SUMS`; file modes depended on the builder's umask; provenance embedded the builder's absolute path; `README.en.md` was missing from the package | `package.sh` requires a clean tree and `HEAD == v$version`; `install.sh` verifies the packaged hash and stages before replacing; modes are normalized; provenance uses a relative artifact path; `README.en.md` is shipped | `bash -n`, package smoke test |
| 8 | P1 (UX) | Questions about `BufWriter::flush` or JSONL append semantics were routed to Execute because of the substrings `write`/`写入`; read-only runs returned a canned authorization refusal | conceptual-question guard plus word-boundary matching for English action verbs | `domain::tests::mode_inference_is_deterministic_and_requirement_aware` |
| 9 | P1 (UX) | Hitting `max_tool_calls` returned a 38-character "tool-call budget exhausted" answer and discarded gathered evidence | one final tool-free answer turn, then the existing schema repair; canned `needs_evidence` only if both fail | `runtime::tests::tool_budget_gets_one_final_tool_free_answer` |

## 3. Notes on the product fixes

- The tool-budget fallback does not execute the over-budget batch. It reuses the
  conversation and evidence already in the journal, and every fallback request
  is journaled.
- If the final answer validates, its own status decides the stop reason
  (`completed` stays `completed`; conditional/needs_evidence become
  `budget_exhausted`).
- The conceptual-question guard skips Execute only when the sentence is a
  question and has no imperative marker (`帮我`, `替我`, `立刻`, `do it`, ...).
  "把文件删掉" and "帮我删掉这个文件？" remain Execute.

## 4. Open findings and accepted residual risk

| # | Severity | Open item | Why it is not fixed here | Mitigation / decision |
|---|---|---|---|---|
| O1 | P1 | `browser_read` validates the initial URL but Chrome re-resolves DNS, follows redirects and loads subresources; an attacker page can reach metadata/loopback targets | a correct fix needs a validating proxy or pinned resolver; that is a larger browser-subsystem change | do not point `browser_read` at untrusted pages; network permission is still required; tracked for 0.98 |
| O2 | P1 | Run journals have no per-event hash chain or external anchor; a local writer can delete a call/observation pair, rewrite an answer or renumber `seq` and still pass `audit` | the org audit has a hash chain, but retrofitting run journals changes the journal format and replay contract | documented in `docs/threat-model.md`; use org audit/export for tamper evidence; planned for a later schema revision |
| O3 | P1 | Cross-builder reproducibility is not established: no pinned `rust-toolchain.toml`, registry hash directories and archive tool versions differ, and `umask` was previously inherited | `CARGO_HOME`/registry remapping and mode normalization were added, but a second builder/host has not reproduced the package | treat the current package as locally reproducible; publish provenance and hashes; do not claim bit-for-bit cross-builder reproducibility |
| O4 | P1 (process) | `check-attribution.sh` is red on local history: commits `2691df9` and `c790f8d` contain a forbidden AI marker | rewriting local history is destructive; the public release uses a rebuilt-history branch, and the owner has not asked to rewrite `main` | public release must be cut from the rebuilt history; `main` remains red until then |
| O5 | P2 | `metrics` can report 100% replay/audit-complete on a journal that `audit` rejects; M5 counts completed runs only; a failed eval case can lose its artifact record | changing metric denominators would invalidate the archived 1h evidence; the known 1h gap is documented | interpret metrics together with `audit`; M5 is completed-run replay, not all-run replay; regenerate evidence after any denominator change |
| O6 | P2 | `resume` has no `--authority`/`--read-only` and trusts the checkpoint request's permissions | needs a checkpoint schema change and CLI surface; not a 0.97 blocker for local single-user use | do not resume journals from untrusted sources; run with `--authority` from the start |
| O7 | P2 | `skill_list`/`delegation_list` are read-flagged but open stores read-write and may create lock files under `~/.1a` during `--read-only` | a true read-only store path is a separate storage change | `--read-only` still prevents model-driven file writes; these lock files are operator-state noise, not user-file writes |
| O8 | P2 | MCP HTTP accepts any HTTPS host, including names that resolve to private/loopback addresses; no public-IP resolution like `fetch_url` | config is operator-supplied, and full resolution/rebinding defense belongs with the browser/network work | only use MCP HTTP endpoints you control; keep bearer tokens scoped/TTL-bound |
| O9 | P2 | Package docs still contain builder absolute paths; CI actions are pinned by tags, not commit SHAs | cosmetic/metadata, not runtime behavior | sanitize in the next packaging pass; pin actions when CI becomes a release input |

## 5. What this audit does and does not prove

It proves:

- four independent reviewers found no P0 in the reviewed tree;
- the nine fixed findings have code-level regression tests or script checks;
- the remaining P1/P2 list is explicit rather than hidden.

It does not prove:

- that no vulnerability exists (a missing entry means "not reviewed", not
  "safe");
- third-party security/compliance certification (SOC 2, ISO 42001, EU AI Act);
- that `browser_read` or run-journal tamper evidence are production-grade;
- that the package is bit-for-bit reproducible on a different builder.

## 6. Reproduction

```bash
cargo fmt --all --check
cargo clippy --offline --all-targets -- -D warnings
cargo test --offline -j 2
./scripts/verify.sh
./scripts/check-attribution.sh   # expected red on local main until rebuilt history
```

Targeted regressions:

```bash
cargo test --offline --lib execution_gate_denies_tools_that_require_approval
cargo test --offline --lib tool_budget_gets_one_final_tool_free_answer
cargo test --offline --lib resume_rejects_a_tool_call_after_the_last_checkpoint
cargo test --offline --lib spawn_is_write_capable_so_read_only_gates_cannot_bypass_it
cargo test --offline --lib mode_inference_is_deterministic_and_requirement_aware
```

## 7. Reviewer checklist for the next external pass

- [ ] Rebuild from the stated commit and verify the binary hash.
- [ ] Confirm no provider secret appears in logs, journals or error output.
- [ ] Attempt unapproved write/terminal actions under `--read-only`, Authority
      and the kill switch; confirm fail-closed.
- [ ] Replay a sample of completed journals and audit them.
- [ ] Attempt the O1 browser SSRF and O2 journal rewrite scenarios.
- [ ] Record P0/P1 findings and an explicit pass/fail decision.
