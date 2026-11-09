# Status v0.5.0

Date: 2026-11-01
Status: controlled experience retrieval and injection

## Added since v0.4.0

- Experience candidates carry applicable cognitive modes and optional expiry time.
- Retrieval returns only `active`, unexpired, mode-matching and scope-matching candidates.
- Experience injection is capped by the remaining context budget.
- Injected guidance is explicitly labeled:

```text
approved but unverified policy guidance, not factual evidence
```

- Every injection journals candidate ids, versions and reasons.
- Draft, approved-only, rejected, rolled-back, out-of-scope and expired candidates are not used.
- CLI supports `--experience-dir` and `--experience-scope`.

## Verification

| Check | Result |
|---|---|
| Unit tests | 32 passed |
| Active-scope retrieval | passed |
| Draft exclusion | passed |
| Out-of-scope exclusion | passed |
| Runtime injection and journaling | passed |
| Checkpoint/resume equivalence | passed |
| CLI run/config/experience/resume | passed |
| Clippy `-D warnings` | passed |

## Requirement comparison

| Requirement | Status |
|---|---|
| L2 candidate lifecycle | implemented |
| Active-only retrieval | implemented |
| Mode/scope/expiry filtering | implemented |
| Context-budget cap | implemented |
| Injection provenance | implemented |
| Held-out quality improvement | pending |
| Real model evaluation | pending |
| Workspace-wide read-only search | pending |

## Known risks

- Scope is currently a literal string; taxonomy and hierarchical scope resolution are pending.
- Candidate content is human-approved but still treated as lower-trust guidance.
- There is no feedback loop proving that an injected candidate improved outcomes.
- Retrieval ranking is recency plus id, not measured usefulness.

## Next requirement

Implement workspace-wide read-only search:

1. Search under configured read roots only.
2. Skip symlinks and common generated directories.
3. Return file, line, excerpt and bounded metadata.
4. Reject hidden control characters in returned text.
5. Bound traversal depth, file count, file size and result count.
6. Add path escape, generated-directory and result-limit tests.
