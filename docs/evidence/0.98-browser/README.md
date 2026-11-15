# 0.98 Browser / CUA Evidence

Date: 2026-11-15  
Gate: G2 (policy + live Chrome)

## Reproduce

```bash
cargo test --offline --lib browser::
cargo test --offline --test browser_e2e
cargo test --offline --test browser_tool_approval
cargo test --offline --test browser_approvals_cli
cargo test --offline --test browser_cli
```

Raw output: `tests.txt`.

## Result

- real Chrome navigate/read/click/type/screenshot/screencast;
- per-task profile, egress proxy, CONNECT/HTTP, DNS pinning/private-IP refusal;
- irreversible click returns `approval_required`; a fake page “Approved”
  button, cross-origin authorize link and download link cannot self-authorize;
- approval is persisted 0600, bound to the DOM descriptor/session/origin,
  expires after 120s and is consumed exactly once; replay is rejected;
- plaintext in a password field is refused; `secret://` entry requires a card
  and the value never appears in tool output/metadata;
- CLI list/approve/deny works; approve without a TTY is refused.

## Known gaps

- no adaptive model-level injection corpus with ASR/utility/false-block;
- no bwrap/Landlock/seccomp wrapper (Chrome sandbox preserved);
- no screenshot masking for secret text fields;
- action approval is not a hard boundary against a model with terminal-write
  permission; run W2 without terminal write.
