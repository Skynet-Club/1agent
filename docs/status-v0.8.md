# Status v0.8.0

Date: 2026-11-01
Status: stable stdio host protocol

## Added since v0.7.0

- `serve` CLI host using newline-delimited JSON over stdin/stdout.
- Host protocol version `3`.
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
- `status` control message for global counters and per-request lifecycle state.
- Ctrl+C cancellation propagates into the active run.
- Example request at `examples/host_request.json`.

## Protocol

Request:

```json
{"protocol_version":3,"request_id":"req-1","task":{}}
```

Response:

```json
{"protocol_version":3,"request_id":"req-1","action":"run","ok":true,"result":{}}
```

Failure:

```json
{"protocol_version":3,"request_id":"req-1","action":"run","ok":false,"error":{"code":"run_failed","message":"..."}}
```

Cancel:

```json
{"protocol_version":3,"request_id":"req-1","action":"cancel"}
```

Cancel acknowledgement:

```json
{"protocol_version":3,"request_id":"req-1","action":"cancel","ok":true}
```

Status:

```json
{"protocol_version":3,"request_id":"status-1","action":"status","target_request_id":"req-1"}
```

Status responses include active/completed/cancelled/failed totals and an optional target state.
```

## Verification

| Check | Result |
|---|---|
| Unit tests | 43 passed |
| Real subprocess host request | passed |
| Invalid JSON followed by a valid request | passed; host survives and continues |
| Two concurrent requests plus invalid input | passed; all 3 responses correlated correctly |
| Queue full plus cancel by request id | passed; target cancelled, peer unaffected |
| Provider inspection with secret environment | passed; env name shown, value absent |
| Host registry active/terminal state | passed |
| Host result semantic audit | passed |
| Existing CLI/config/experience/resume/eval suites | passed |
| Clippy `-D warnings` | passed |

Release artifacts:

```text
Linux binary:   fb45d56a4e234d5993696df8cbee6a263b083dc7f08e0dcb935d91562736bf35
Windows binary: f2b7728e5c22a7c54923a85ac274b46a3418fdb26ead9fa53baf996f7d1b6151
Linux tar.gz:   a1fab6ec557dba73bd0971cabb30df19eb4eab7d06249c79cd38c470b219c67d
Windows zip:    133d8f408a56c05c3c57c1fa0a5b3273b385e5603c63aa718e7812df3932f531
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
| Host status/lifecycle query | implemented with bounded 256-entry recent history |
| Desktop/terminal adapter | pending |
| Real-provider quality | unverified, no credentials |

## Known risks

- Stdin/stdout transport is local-only; authentication belongs to future transports.
- Provider credentials are inherited from the host environment.

## Next requirement

Add run-history discovery backed by authoritative journals:

1. List recent completed runs with run id and stop reason.
2. Resolve request ids from host status to journal paths.
3. Keep in-memory status disposable and rebuildable from journals.
4. Bound history scans and avoid loading observation bodies.
5. Test restart recovery of discoverable terminal history.
