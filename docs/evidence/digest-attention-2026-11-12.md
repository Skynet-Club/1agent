# Digest Attention Summary — Evidence (2026-11-12)

Status: **passed on this machine**  
Scope: `1agent digest --inbox-store/--handoff-store/--recipient`.

## Commands

```bash
cargo test --test digest_cli
cargo clippy --all-targets -- -D warnings
```

## Expected results

```text
running 2 tests (tests/digest_cli.rs)
test digest_counts_inbox_and_open_handoffs ... ok
test digest_rejects_recipient_without_handoff_store ... ok

test result: ok. 2 passed; 0 failed
```

## What this proves

- One open inbox item and one open handoff to bob produce
  `inbox.open: 1` and `handoffs.open: 1`.
- Accepting the handoff drops the handoff count to zero.
- `--recipient` without `--handoff-store` exits non-zero.
- With a Hacked verification record, `--verification-store` reports
  hacked=1, resolved=1 and 10000 bps.

## What this does not prove

- That the recipient identity is authenticated.
- That inbox/handoff duplication is removed.
- That the digest is scheduled or delivered.
- That the maintenance loop runs.
- Cross-platform behavior; tests ran on Linux only.

## Files

- `src/main.rs` (`digest` flags and counts)
- `tests/digest_cli.rs`
- `docs/adr/0075-digest-attention-summary.md`
- `docs/features.md`, `docs/features.en.md`, `docs/claims.md`, `CHANGELOG.md`
