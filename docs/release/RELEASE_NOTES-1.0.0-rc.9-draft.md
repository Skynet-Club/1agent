# 1Agent 1.0.0-rc.9 — capability compiler P0 (draft)

Date: 2026-11-18  
Status: **local code-level release candidate; not published, not GA**  
Scope: 1.0.0-rc.8 plus the open-world capability compiler

This is a working release note, not a GitHub release announcement. External
audit, real Windows/macOS GA, marketplace responsibility and R1–R9 user metrics
are **blocked** and must not be presented as passed.

## 1. What changed in rc.9

- Added an open-world capability compiler. A task is compiled into a
  `TaskContract` (objective, deliverable, capability requirements, constraints,
  acceptance), then resolved against declarative `CapabilityManifest` entries.
- Added `capabilities/manifests.json` with seed manifests; a new tool or skill
  only needs a manifest, not a task-type or tool-name switch.
- Added the read-only `resolve_capability` tool. It returns selected installed
  candidates, alternatives with reasons, and named gaps; it never installs or
  executes anything.
- Unknown capabilities are reported as gaps, not silently converted to text.
- The system prompt now requires `resolve_capability` before a non-text
  deliverable and forbids marking `completed` without the required artifact.
- Added `docs/capability-compiler.md` and tests for diagram gaps, unknown
  capabilities, installed overrides and a new manifest resolving a new
  capability without code changes.

## 2. Behavior smoke

A real-model architecture-diagram task called:

```text
resolve_capability → capability_search → write_file → preview_file →
browser → browser → acceptance_gate
```

`resolve_capability` reported no installed `render` capability for
`image/svg+xml` and listed `archify` as an uninstalled alternative. The model
then built an SVG, previewed it, ran a browser audit and `acceptance_gate`
accepted 5 criteria. The final answer was `needs_evidence` because the default
step budget was exhausted before finalization. Evidence:
`docs/evidence/capability-compiler-2026-11-18/`.

## 3. Verification (local)

| Check | Result |
|---|---|
| `./scripts/verify.sh` | pass; 1657 total, 0 failed, 13 ignored |
| claims table | well-formed; 262 checked (238 verified / 16 partial / 8 no evidence) |
| Resolver tests | diagram gap, unknown capability, installed override, new manifest |
| Behavior smoke | `resolve_capability` called; artifact produced; final status limited by step budget |

## 4. Known limits / blocked

- P0 is the resolver/tool/prompt layer. Runtime preflight enforcement,
  artifact verifier and install-approval flow are P1.
- The behavior smoke ended `needs_evidence` despite an accepted artifact
  because the step budget ran out before finalization; this is a P1 runtime
  issue.
- Human calibration, 8h DeepSeek quality, external audit, real Windows/macOS
  GA, marketplace and R1–R9 remain blocked.
- Development history contains an AI byline marker; a public release needs a
  clean release branch or an amended attribution policy.

## 5. Local assets

Built from tag `v1.0.0-rc.9` → `fe0c739` with `./scripts/package.sh --linux-only`;
tree clean, `reproducible=true`.

| Asset | Value |
|---|---|
| `1agent-1.0.0-rc.9-linux-x86_64.tar.gz` | SHA-256 `914ec66875cdd8118697d65b85476b04d8b4c45cfdab552bd9279cf7bb97a26a` |
| Packaged binary | SHA-256 `ec3bef264921e8d65504dd3d464ea337a48921c0c2368d5411a29366c31a4e92` |
| Provenance | `linux-x86_64.json`, commit `fe0c739`, `git_dirty: false` |
| Smoke | SHA256SUMS OK; `--version` 1.0.0-rc.9; install to `~/.local` OK |

Evidence: `docs/evidence/release-1.0.0-rc.9-2026-11-18.md`. No GitHub release
and no push were performed.
