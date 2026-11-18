# 1Agent 1.0.0-rc.11 — capability compiler P1/P2 (draft)

Date: 2026-11-18  
Status: **local code-level release candidate; not published, not GA**  
Scope: 1.0.0-rc.10 plus runtime capability enforcement, artifact verification,
install approval and capability-path memory

## 1. What changed

- **Runtime preflight**: before the model loop, the runtime compiles the task
  contract and resolves capabilities. An unresolvable required capability
  returns `needs_authorization` without calling the model. A resolved path is
  added to the journaled `known_facts`.
- **Artifact verifier**: `verify_artifact` checks existence, size, MIME,
  lightweight schema, SHA-256 and optional loopback preview HTTP 200.
  `CompletionGate` records verified artifacts.
- **Install proposal + human approval**:
  `propose_capability_install` returns a structured proposal with exact
  command/license/source and `approval_required: true`.
  `capability approve-install --yes` records the approver and command hash in
  `capability-approvals.jsonl` with `executed: false`; it never runs the install
  command. A human runs the command.
- **Capability-path memory**: `capability-paths.jsonl` records
  requirement → selected manifest → status → artifact_verified. Successful
  paths add a small explainable boost and can reselect among installed
  candidates. No task-type labels are stored.
- **CLI**: `capability resolve`, `capability propose-install`,
  `capability approve-install`.

## 2. Verification

| Check | Result |
|---|---|
| `./scripts/verify.sh` | pass; 1666 total, 0 failed, 13 ignored |
| claims | 263 checked (239 verified / 16 partial / 8 no evidence) |
| preflight gap | 3D task returns `needs_authorization` with no model response |
| artifact verifier | SVG/MIME/schema/sha256 pass; wrong hash/schema fail |
| install flow | proposal and approval separated; approval ledger `executed:false` |
| completion gate | verified artifact recorded |
| capability memory | path record and boost round-trip |

## 3. Limits

- Preflight uses declared deliverable hints; an entirely unknown artifact
  format with no hint still needs an explicit `resolve_capability`.
- Approval records human approval but does not execute or verify the install.
- P2 is a small path-history boost, not a learned planner.
- Live P2 smoke ended in answer-repair failure after an accepted artifact; the
  path was not recorded because outcome status was `needs_evidence`.
- Human calibration, 8h DeepSeek, external audit, real Windows/macOS, R1–R9
  remain blocked.

## 4. Local assets

Built from tag `v1.0.0-rc.11` → `f352ba8` with `./scripts/package.sh --linux-only`;
tree clean, `reproducible=true`.

| Asset | Value |
|---|---|
| `1agent-1.0.0-rc.11-linux-x86_64.tar.gz` | SHA-256 `3f8dc770ece6dff085b4b58bf8e2ec4a08201b90b878fcd6aedf1b80bac3c8bd` |
| Packaged binary | SHA-256 `d469f9cfae365beec954435739e253f0e19d83e1c80749af4c7c9d8d6a7bda56` |
| Provenance | `linux-x86_64.json`, commit `f352ba8`, `git_dirty: false` |
| Smoke | SHA256SUMS OK; `--version` 1.0.0-rc.11; install to `~/.local` OK |

Evidence: `docs/evidence/release-1.0.0-rc.11-2026-11-18.md`. No GitHub release
and no push were performed.
