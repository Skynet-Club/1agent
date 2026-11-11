# ADR 0058: Team Role and Scope Policy

- Status: accepted
- Date: 2026-11-12

## Context

A shared partner needs a team permission model before it can be used by more
than one person. The runtime already has per-actor authority and approvals,
but no role vocabulary for a team.

## Decision

1. Add `src/team.rs` with:
   - `TeamRole`: `owner`, `approver`, `member`, `viewer`;
   - `TeamAction`: `read`, `write`, `approve`, `manage`;
   - `TeamMember { member_id, role, scopes }`;
   - `TeamPolicy { team_id, members }`.
2. Role matrix:
   - owner: all actions;
   - approver: read + approve;
   - member: read + write;
   - viewer: read.
3. An empty scope list means all scopes. A non-empty list must contain the
   requested scope exactly.
4. Unknown actors, disallowed actions and ungranted scopes are denied.
5. `intersect_scopes(requested, granted)` returns the sorted, deduplicated
   intersection for delegation.
6. Add `1agent team-check --role ... --action ... [--scope ...]
   [--granted-scope ...] [--json]`. A denied check exits non-zero.
7. This is a policy module, not an identity provider. The caller must supply an
   authenticated actor and scope.

## Consequences

- A shared partner can enforce role and scope before acting.
- The policy is testable without an IdP or network.
- Real team identity, invitations, org audit and cross-team delegation remain
  separate slices.
- The role matrix is deliberately small; adding a role requires an explicit
  decision.

## Evidence

```bash
cargo test team --lib
cargo test --test team_check_cli
```

The tests cover owner/approver/member/viewer, unknown actors, ungranted scopes
and scope intersection.
