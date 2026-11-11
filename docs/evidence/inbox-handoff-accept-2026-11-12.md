# Inbox Handoff Acceptance — Evidence (2026-11-12)

Status: **passed on this machine**  
Scope: `1agent inbox resolve` for `handoff:<id>` items.

## Commands

```bash
cargo test --test handoff_inbox_cli
cargo clippy --all-targets -- -D warnings
```

## Expected results

```text
running 3 tests (tests/handoff_inbox_cli.rs)
test handoffs_project_into_the_recipient_inbox_once ... ok
test inbox_resolve_accepts_the_linked_handoff ... ok
test recipient_requires_a_handoff_store ... ok

test result: ok. 3 passed; 0 failed
```

## What this proves

- An open handoff projects into bob's inbox as `handoff:h1`.
- Resolving it as approved without `--handoff-store/--actor` fails.
- A wrong actor fails and leaves the handoff open.
- The addressee's approval returns item `approved` and handoff `accepted` with
  `accepted_by: bob`.
- The inbox then has no open items.

## What this does not prove

- Atomicity across the handoff store and inbox store.
- That the actor identity is authenticated.
- Rejection/decline semantics for a handoff.
- Notifications or presence.
- Cross-platform behavior; tests ran on Linux only.

## Files

- `src/main.rs` (`inbox resolve --handoff-store/--actor`)
- `tests/handoff_inbox_cli.rs`
- `docs/adr/0073-inbox-handoff-acceptance.md`
- `docs/architecture.md`, `docs/features.md`, `docs/features.en.md`,
  `docs/claims.md`, `CHANGELOG.md`
