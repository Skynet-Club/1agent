# 0.98 Skills Evidence

Date: 2026-11-15  
Gate: G4 (validator, eval, sandbox, lock)

## Reproduce

```bash
cargo test --offline --lib skill_
cargo test --offline --test skill_validate_cli
cargo test --offline --test skill_canary_cli
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

## Known gaps

- blind comparator/held-out split are not yet a mandatory activation gate;
- no adaptive malicious-skill corpus;
- capture → real model eval → canary → rollback needs a provider-backed run.
