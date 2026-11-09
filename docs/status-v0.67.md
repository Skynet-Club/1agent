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
| Clippy `-D warnings` | passed |
| Clean-source package gate | passed |

## Known limitations

- Experience event file remains append-only JSONL without a file-wide hash chain.
- Operators with filesystem access can rewrite and consistently re-hash all derived state.
- Legacy digest-free candidates cannot prove source integrity.
- Source journals are external to the experience store.

## Verified clean release

```text
source commit: fbbeaa9cfb229a8a3e13243ca29b3fb096bebd88
git dirty: false

Linux binary:
c93814fceab663a8472a6f1d518ff338d34704b035a965b72262d86f16bac322

Windows binary:
fbd77633475b30bb166662b7d2863ac59b1dff08de50a0c0928804fc78289f8f

Linux tar.gz:
a3dd28257c99101b8cbdbc1a013d12d6db70f9b17ed4b69fe942fdec2a1ac257

Windows zip:
3f8a1937c536267fc2932c3dfb917edf9772b7ffe4c398ef5bf6b2f5030591a3

Release manifest:
bfb336bfb01f6625d7fe0838bbeb6d5e0b6775a55dda42722429cecb4f5ea84b
```

## Next requirement

Bound experience text, modes and evidence-reference volume:

1. cap title, content and scope length;
2. cap evidence sequence count;
3. reject duplicate/empty applicable modes;
4. keep retrieval context budget unchanged;
5. test boundary and oversized proposals.
