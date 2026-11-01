# Status v0.67.0

Date: 2026-11-02
Status: experience-store replay hardened

## Changes since v0.66.0

- Proposed events must decode as unverified drafts.
- Candidate version must be positive.
- Source run ID must not be nil.
- Evidence sequences must be non-empty and unique.
- Bound evidence digest must be 64 hexadecimal characters.
- Applicable cognitive modes must be non-empty.
- Supersession replay rechecks version increment.
- Activation replay rechecks active-scope uniqueness.
- Legacy candidates without digests remain readable.

## Verification

| Check | Result |
|---|---|
| Unit tests | 113 passed |
| Integration / E2E | passed offline |
| Core evaluation | 6/6 passed |
| tampered digest | rejected |
| forged Active proposal | rejected |
| zero candidate version | rejected |
| version increment replay | checked |
| active-scope replay | checked |
| legacy digest-free candidate | accepted |
| Clippy `-D warnings` | pending |
| Clean-source package gate | pending |

## Known limitations

- Experience event file remains append-only JSONL without a file-wide hash chain.
- Operators with filesystem access can rewrite and consistently re-hash all derived state.
- Legacy digest-free candidates cannot prove source integrity.
- Source journals are external to the experience store.

## Next requirement

Bound experience text, modes and evidence-reference volume:

1. cap title, content and scope length;
2. cap evidence sequence count;
3. reject duplicate/empty applicable modes;
4. keep retrieval context budget unchanged;
5. test boundary and oversized proposals.
