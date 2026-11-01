# Status v0.8.0

Date: 2026-11-01
Status: stable stdio host protocol

## Added since v0.7.0

- `serve` CLI host using newline-delimited JSON over stdin/stdout.
- Host protocol version `1`.
- Stable client request id echoed in every response.
- Structured `HostResponse` with either `RunResult` or `HostError`.
- Maximum request size of 1 MiB.
- Invalid JSON, unsupported protocol and empty request ids are rejected without crashing the host.
- One process can reuse one configured AgentRuntime for multiple sequential requests.
- Ctrl+C cancellation propagates into the active run.
- Example request at `examples/host_request.json`.

## Protocol

Request:

```json
{"protocol_version":1,"request_id":"req-1","task":{}}
```

Response:

```json
{"protocol_version":1,"request_id":"req-1","ok":true,"result":{}}
```

Failure:

```json
{"protocol_version":1,"request_id":"req-1","ok":false,"error":{"code":"run_failed","message":"..."}}
```

## Verification

| Check | Result |
|---|---|
| Unit tests | 41 passed |
| Real subprocess host request | passed |
| Invalid JSON followed by a valid request | passed; host survives and continues |
| Host result semantic audit | passed |
| Existing CLI/config/experience/resume/eval suites | passed |
| Clippy `-D warnings` | passed |

Release artifacts:

```text
Linux binary:   c4755b7ca7b7cb1717cd227120404b38d9aa200482fa445f9e7622d0d4f67580
Windows binary: 6f279b44fb5b86098fd14cf5ac805459797d9852f6d95a1f1c2f5a13143ccac2
Linux tar.gz:   450a9d0f883548beebe0ba20367d385a9a5828635ccc18cc37599c7c7e44350d
Windows zip:    c96490f87508b62f22b0ba618c16701d12d97da4df23999a955515a761844413
```

## Requirement comparison

| Requirement | Status |
|---|---|
| Core/host separation | implemented for CLI and stdio host |
| Stable protocol version | implemented |
| Request correlation | implemented |
| Structured errors | implemented |
| Sequential multi-request host | implemented |
| Concurrent host requests | pending |
| Desktop/terminal adapter | pending |
| Real-provider quality | unverified, no credentials |

## Known risks

- The host is sequential and one slow request blocks the next.
- There is no backpressure or per-request cancellation control beyond global Ctrl+C.
- Stdin/stdout transport is local-only; authentication belongs to future transports.
- Provider credentials are inherited from the host environment.

## Next requirement

Add per-request cancellation and concurrent host dispatch with bounded concurrency:

1. Add request ids to cancellation controls.
2. Admit up to a configured number of simultaneous runs.
3. Return structured queue-full and cancelled errors.
4. Keep journal writes independent and run-scoped.
5. Test cancellation and two concurrent requests against a deterministic slow provider.
