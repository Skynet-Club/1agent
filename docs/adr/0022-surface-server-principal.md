# ADR 0022: Surface HTTP Principal and Context Scoping

- Status: accepted
- Date: 2026-11-08

## Context

The Surface v0 loopback server accepted `AuthContext` from the request envelope. Any
holder of the bearer token could claim another actor and arbitrary scopes, making every
`auth.require` check client-controlled. The same transport also exposed a context summary
that included memory from every bot scope, and the production `SurfaceService::open`
constructor kept the action log in memory only.

## Decision

1. The bearer token identifies a server-side principal. The server constructs
   `AuthContext` from its own configuration (`--actor-id` and repeated `--scope`), and
   request-body auth is ignored. A request cannot widen its actor or scopes.
2. `ContextSummary` requires a `bot_id`, requires both `context:read` and `memory:read`,
   and only includes memory records whose scope matches that bot. Goal, thread and ticket
   nodes remain single-tenant context for now.
3. `SurfaceService::open` opens a sibling `<graph-stem>-action-log.jsonl` through
   `GovernedTaskGraph::open_with_action_log`, so security actions survive a restart and
   two stores in the same directory do not share a journal.
4. The HTTP server rejects a missing or invalid bearer token before dispatch; health and
   OpenAPI metadata remain unauthenticated because they expose no tenant data.

## Consequences

- Operators can limit a server instance with `--scope`; the default `*` preserves the
  existing single-operator development behavior but is not a multi-tenant boundary.
- Body envelopes remain wire-compatible but their `auth` field is no longer authoritative.
- Per-bot authorization for goals/tickets/threads and multi-tenant isolation are not
  claimed; a future ADR must define them before exposing one server to multiple tenants.
- The action log is now a second append-only file beside the task graph; backups and
  operational runbooks must include both.
