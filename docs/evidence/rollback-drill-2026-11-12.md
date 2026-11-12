# Journal Rollback Drill — Evidence (2026-11-12)

Status: **local drill passed (one-way compatibility confirmed)**
Scope: read a 0.97 journal with the published 0.96.1 binary; read a 0.96.1
journal with the current 0.97 binary.

## Setup

```text
published binary: /home/skynet/releases/1agent-v0.96.1/1agent-0.96.1-linux-x86_64.tar.gz
                  -> 1agent 0.96.1
current binary:   target/release/first-principles-agent
                  -> 1agent 0.96.1 (local 0.97 tree)
```

Both runs used the deterministic `mock` provider and the same read-only task:

```bash
<binary> run --task "@read-file:/home/skynet/1-agent/README.md" \
  --provider mock --journal-dir <dir> --read-root /home/skynet/1-agent
```

The current journal contains these event types:

```text
answer_produced, checkpoint, model_request_delta, model_requested,
model_responded (2), route_planned, route_selected (2), run_started,
run_stopped, tool_called, tool_capabilities_advertised, tool_observed
```

## Results

| Reader | Journal | Exit | Result |
|---|---|---|---|
| current 0.97 | 0.96.1 journal | 0 | audit ok, 14 events, no issues |
| current 0.97 | 0.97 journal | 0 | audit ok, 14 events, no issues |
| published 0.96.1 | 0.97 journal | 1 | cannot read journal |

The published binary fails before it reaches the new event variant:

```text
journal line 1 is invalid: unknown field `max_cost_micros`, expected one of
`max_steps`, `max_tool_calls`, `max_provider_attempts`, ... at line 1 column 737
```

The new journal also contains `model_request_delta` (ADR-0122), which 0.96.1
does not know. The rollback boundary is therefore broader than the delta
itself: a 0.97 journal cannot be consumed by the published 0.96.1 binary at
all.

## What this proves

- Old journals remain readable by the current binary.
- A 0.97 journal is not readable by the published 0.96.1 binary; rollback
  requires finishing or archiving new journals first.
- The drill used one published binary and one deterministic read-only task, not
  a production upgrade.

## What this does not prove

- A full production upgrade/downgrade with live sessions, credentials and
  config migrations.
- That every 0.97 journal fails at the same field; the first failure is the
  `RunStarted` budget schema, and later variants would also be unknown.
- A rollback of the on-disk session directory, config or kill switch; those
  are documented in `docs/rollback-v0.97.md` but were not exercised here.
