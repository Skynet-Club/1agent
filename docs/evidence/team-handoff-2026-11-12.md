# Team Handoff — Evidence (2026-11-12)

Status: **passed on this machine**  
Scope: `handoff::HandoffStore` and `1agent handoff`.

## Commands

```bash
cargo test handoff --lib
cargo test --test handoff_cli
cargo clippy --all-targets -- -D warnings
```

## Expected results

```text
running 4 tests
test handoff::tests::create_and_accept_survive_reopen ... ok
test handoff::tests::duplicate_and_ungrounded_handoffs_are_rejected ... ok
test handoff::tests::list_is_sorted_and_unknown_accept_fails ... ok
test handoff::tests::only_the_addressee_can_accept_once ... ok

test result: ok. 4 passed; 0 failed

running 2 tests (tests/handoff_cli.rs)
test handoff_create_accept_and_list ... ok
test handoff_rejects_ungrounded_and_self_handoffs ... ok

test result: ok. 2 passed; 0 failed
```

## What this proves

- A grounded handoff is created as open and survives reopening.
- The sender cannot accept; the addressee can; a second accept is rejected.
- An unknown id, duplicate id, missing evidence, empty summary and self-handoff
  are rejected.
- Listing is ordered by creation time and can filter with `--open` or
  `--to NAME`.
- The sender or addressee can cancel an open handoff with a reason; a
  cancelled handoff is terminal, cannot be accepted, and is excluded by
  `--open`.
- The CLI create/accept/list flow matches the store semantics.

## What this does not prove

- That `from` is authenticated; the store is writable by any local user who
  can reach it.
- Live shared context, presence or notifications.
- Per-team access control over the handoff store.
- Cryptographic binding to the organization audit index.
- Encryption at rest.
- Cross-platform behavior; tests ran on Linux only.

## Files

- `src/handoff.rs`
- `src/main.rs` (`handoff create|accept|list`)
- `tests/handoff_cli.rs`
- `docs/adr/0068-team-handoff.md`
- `docs/architecture.md`, `docs/features.md`, `docs/features.en.md`,
  `docs/claims.md`, `CHANGELOG.md`
