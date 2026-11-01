# Status v0.8.0

Date: 2026-11-01
Status: stable stdio host protocol

## Added since v0.7.0

- `serve` CLI host using newline-delimited JSON over stdin/stdout.
- Host protocol version `2`.
- Stable client request id echoed in every response.
- Structured `HostResponse` with either `RunResult` or `HostError`.
- Maximum request size of 1 MiB.
- Invalid JSON, unsupported protocol and empty request ids are rejected without crashing the host.
- One process can reuse one configured AgentRuntime for multiple requests.
- Bounded concurrent dispatch with configurable `--max-concurrency` from 1 to 32.
- Responses may arrive out of order and are correlated by request id.
- Cancel control message keyed by request id.
- Immediate `queue_full` response when concurrency is saturated.
- Immediate `duplicate_request` response for an active request id.
- Ctrl+C cancellation propagates into the active run.
- Example request at `examples/host_request.json`.

## Protocol

Request:

```json
{"protocol_version":2,"request_id":"req-1","task":{}}
```

Response:

```json
{"protocol_version":2,"request_id":"req-1","action":"run","ok":true,"result":{}}
```

Failure:

```json
{"protocol_version":2,"request_id":"req-1","action":"run","ok":false,"error":{"code":"run_failed","message":"..."}}
```

Cancel:

```json
{"protocol_version":2,"request_id":"req-1","action":"cancel"}
```

Cancel acknowledgement:

```json
{"protocol_version":2,"request_id":"req-1","action":"cancel","ok":true}
```

## Verification

| Check | Result |
|---|---|
| Unit tests | 42 passed |
| Real subprocess host request | passed |
| Invalid JSON followed by a valid request | passed; host survives and continues |
| Two concurrent requests plus invalid input | passed; all 3 responses correlated correctly |
| Queue full plus cancel by request id | passed; target cancelled, peer unaffected |
| Host result semantic audit | passed |
| Existing CLI/config/experience/resume/eval suites | passed |
| Clippy `-D warnings` | passed |

Release artifacts:

```text
Linux binary:   77e9205a7a871f2677b1eb6ac723fcf651c3db0c3ff64ad3da48ad802e14b2c7
Windows binary: 1e9fff1aa2ccd9acf681cfe366d8686340277efddd3dee5eeaf76b25cc17ff06
Linux tar.gz:   23ef517d33d4e2ccb238aa9e2bb6b521faddc5f2a9b880c97cdc5b3259dae702
Windows zip:    1ef5f579a3d25051ba0948b23c67cdb6370ce51c6da59e45e5ff07f058433ba4
```

## Requirement comparison

| Requirement | Status |
|---|---|
| Core/host separation | implemented for CLI and stdio host |
| Stable protocol version | implemented |
| Request correlation | implemented |
| Structured errors | implemented |
| Sequential multi-request host | implemented |
| Bounded concurrent host requests | implemented |
| Per-request cancellation | implemented |
| Queue admission errors | implemented |
| Desktop/terminal adapter | pending |
| Real-provider quality | unverified, no credentials |

## Known risks

- Stdin/stdout transport is local-only; authentication belongs to future transports.
- Provider credentials are inherited from the host environment.

## Next requirement

Add a persistent host status and lifecycle endpoint:

1. Report active, completed and cancelled request counts.
2. Query a request by id without loading its full journal.
3. Keep status bounded and non-authoritative relative to the run journal.
4. Preserve request id reuse after terminal state.
5. Test status visibility and cleanup under concurrent runs.
