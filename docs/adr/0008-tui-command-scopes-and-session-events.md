# ADR 0008: Host-Aware TUI Command Scopes and Session Events

Status: accepted

Date: 2026-11-03

## Context

1A uses a TUI interaction model inspired by Codex, but it is not a general-purpose
Codex clone. Its differentiating capability is production-line control inside Skynet
Workshop.

Showing task decomposition, dispatch, supervision and acceptance commands in a standalone
TUI would blur the product boundary and expose commands that have no Master state or
Workshop execution target. The command drawer therefore needs an explicit host context.

Session metadata also needs an audit trail for local TUI actions. Run journals cover
model-visible input and tool activity, while Master journals cover coordination state.
Neither is the correct place to record every local command rejection or session lifecycle
event.

## Decision

Introduce a `HostBinding` with three states:

- `Standalone`;
- `WorkshopMaster`;
- `WorkshopWorker`.

Commands declare a scope:

- `Core`: available in standalone and Workshop;
- `Master`: available only to Workshop Master.

The TUI combines scope, lifecycle stage and run availability when constructing the command
drawer. A typed command uses the same resolution logic. UI visibility never grants
authority; future Master handlers must revalidate Master state and the Workshop adapter.

The initial compatibility binding uses the existing Workshop environment variables. A
versioned explicit context is accepted when all of host, role, version, agent, runtime and
session agree. Conflicting or incomplete signals fail closed to standalone.

Upgrade TUI session metadata to version 2 while retaining read compatibility with version
1. Add an append-only session event log with a schema version, contiguous sequence,
timestamp and typed session events. Event records are control-safe and exclude secrets.

## Consequences

- Standalone conversations remain focused on general 1A capabilities.
- Workshop Master can grow a stage-aware production console without cluttering other modes.
- Command metadata can exist before a handler is complete; incomplete Master commands are
  visibly marked as under development and cannot execute.
- Session actions and rejections become independently auditable.
- Host environment is a routing and presentation signal, not a security boundary.
- Future command handlers must remain thin application adapters over existing runtime,
  session and Master services.

## Verification

- standalone and Worker surfaces contain no Master commands;
- Workshop Master surfaces contain only commands for the current stage;
- typed Master commands are rejected outside Workshop Master;
- conflicting explicit host signals fall back to standalone;
- session v1 files remain readable;
- event sequence continuation and non-contiguous rejection are tested;
- line mode and interactive mode share command resolution;
- the full test, clippy, formatting, rustdoc and PTY acceptance gates pass.
