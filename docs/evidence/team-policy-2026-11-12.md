# Team Role Policy — Evidence (2026-11-12)

Status: **passed on this machine**  
Scope: `team::authorize`, `team::intersect_scopes` and `1agent team-check`.
This is a policy module, not an identity provider.

## Commands

```bash
cargo test team --lib
cargo test --test team_check_cli
cargo clippy --all-targets -- -D warnings
```

## Expected results

```text
running 4 tests
test team::tests::owner_can_do_everything ... ok
test team::tests::roles_are_separated ... ok
test team::tests::unknown_actor_and_ungranted_scope_are_denied ... ok
test team::tests::scope_intersection_is_sorted_and_deduplicated ... ok

test result: ok. 4 passed; 0 failed

running 2 tests (tests/team_check_cli.rs)
test team_roles_and_scopes_are_enforced ... ok
test team_check_denies_and_rejects_bad_input ... ok

test result: ok. 2 passed; 0 failed
```

## What this proves

- Owner can read/write/approve/manage.
- Approver can read/approve but not write; member can read/write but not
  approve; viewer is read-only.
- Unknown actors and ungranted scopes are denied.
- `intersect_scopes` returns a sorted, deduplicated intersection.
- `team-check` exits non-zero on a denied decision and rejects invalid roles.

## What this does not prove

- That a real identity provider authenticates the actor; the caller supplies
  the actor id.
- That team invitations, org audit and cross-team delegation work.
- That the role matrix fits every organization; it is deliberately small.
- Cross-platform behavior; the tests ran on Linux only.

## Files

- `src/team.rs`
- `src/main.rs` (`team-check`)
- `tests/team_check_cli.rs`
- `docs/adr/0058-team-role-policy.md`
- `docs/architecture.md`, `docs/features.md`, `docs/features.en.md`,
  `docs/claims.md`, `CHANGELOG.md`
