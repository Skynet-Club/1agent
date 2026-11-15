# 0.98 Skills Evidence

Date: 2026-11-15  
Gate: G4 (validator, eval, sandbox, lock)

## Reproduce

```bash
cargo test --offline --lib skill_
cargo test --offline --test skill_validate_cli
cargo test --offline --test skill_canary_cli
cargo test --offline --test skill_malicious_corpus
```

Raw output: `tests.txt`.

## Result

- `SKILL.md` validator rejects symlinks, path escapes, oversize files and
  instruction-override markers;
- execution eval hashes outputs instead of storing them; activation requires a
  passing report or explicit audited `--force`;
- persistent canary and candidate/demonstration stores;
- blind A/B comparator with seeded arms, output hashes and Wilson 95% lift;
  `skill compare` exits nonzero unless the conservative lift is positive;
- deterministic held-out split;
- active-version lockfile (`skill lock` / `verify-lock`) detects exact drift;
- `skill run-script` executes explicitly selected scripts under bwrap with no
  network, no host home, read-only runtime/skill dir, tmpfs `/tmp`, dropped
  capabilities, timeout and output caps; bwrap absence fails closed.
- `tests/skill_malicious_corpus.rs` (8/8) covers frontmatter injection,
  hidden zero-width instructions, secret-like tokens, oversized/nested
  bundles, asset symlinks, mutable external URLs (warning + count) and script
  bundles (warning + `has_scripts`, never auto-executed). `scan_text` now
  rejects invisible/bidi format characters that could hide a marker; ZWJ is
  left alone for emoji.

## Known gaps

- blind comparator/held-out split are not yet a mandatory activation gate;
- the corpus is structural/adversarial fixtures, not an adaptive
  model-generated suite, and scanning is a signal, not an antivirus guarantee;
- capture → real model eval → canary → rollback needs a provider-backed run.
