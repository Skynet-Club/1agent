# License and Distribution Policy

## Project license

1Agent is released under the **MIT License**. The authoritative text is
`LICENSE`; the Cargo package declares `license = "MIT"` and SPDX `MIT`.

## Contributions

Unless a separate written agreement exists, contributions are accepted under the
same MIT terms. There is no CLA. Contributors must have the right to submit the
code and must not add dependencies with incompatible licenses.

## Third-party code

- The adapted-code notice for OpenAI Codex / Ratatui is in
  `THIRD_PARTY_NOTICES.md`.
- The generated dependency inventory is `THIRD_PARTY_LICENSES.md`, covering the
  Linux and Windows registry dependency sets.
- The Apache-2.0 text is in `LICENSES/Apache-2.0.txt`.
- Other upstream license texts are published with their crates. Binary
  distributions must ship `LICENSE`, `THIRD_PARTY_NOTICES.md`,
  `THIRD_PARTY_LICENSES.md` and the `LICENSES/` directory.

## Release obligations

Before a public binary release:

1. Re-generate the dependency inventory from the exact locked dependency graph
   for every published target.
2. Confirm no dependency has an incompatible license.
3. Ship the notices and license texts inside the archive, not only in the source
   repository.
4. Obtain the formal third-party/legal review required by R1.

This document is an engineering policy, not legal advice.
