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
- `providers` command for secret-safe configuration inspection.
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
| Provider inspection with secret environment | passed; env name shown, value absent |
| Host result semantic audit | passed |
| Existing CLI/config/experience/resume/eval suites | passed |
| Clippy `-D warnings` | passed |

Release artifacts:

```text
Linux binary:   367a47cea6d8c0062806c0abe24b0eecf170850138b857f9bfd8075cbb90afe1
Windows binary: 49711d85941fd385848de8e87f8a819a872eee757f8f9d70ed201d7f9f6a3982
Linux tar.gz:   2f3bfe36fba95269d008978817e386e3c4ce13eaaa63fd506b94d38a47a740d3
Windows zip:    3e9bced39a1362e10e9fbc868c8ab9e2b713ff650452e9291f024d9e1b716e84
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
