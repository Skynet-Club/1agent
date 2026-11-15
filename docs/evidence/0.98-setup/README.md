# 0.98 Setup / CLI Evidence

Date: 2026-11-15  
Gate: G1 (automated portion)

## Reproduce

```bash
cargo test --offline --test setup_cli
cargo test --offline --lib config::tests
```

Raw output: `tests.txt`; manual fresh-home setup/status/run/doctor transcript:
`fresh-home.txt`; interactive wizard PTY evidence: `wizard-e2e.md` (captures:
`wizard-*.txt`, harness: `tools/`).

## Result

- setup state machine, `--check`, `--non-interactive`, `--resume`, API-key
  stdin, argv rejection, 0600 key files, probe failure paths, config v2 parsing.
- 11 setup CLI tests + 12 config tests passed;
- a real fresh home with `--model wrong` still persisted the deterministic
  identity `offline-v1`, and `status`/`run`/`doctor` used the default config.

## Not yet evidenced

- five-user timing/feedback;
- final release-package install of completions/man (packaging requires a clean
  commit/tag).
