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
- One process can reuse one configured AgentRuntime for multiple requests.
- Bounded concurrent dispatch with configurable `--max-concurrency` from 1 to 32.
- Responses may arrive out of order and are correlated by request id.
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
| Two concurrent requests plus invalid input | passed; all 3 responses correlated correctly |
| Host result semantic audit | passed |
| Existing CLI/config/experience/resume/eval suites | passed |
| Clippy `-D warnings` | passed |

Release artifacts:

```text
Linux binary:   4161d4bfeee75e1485d05a8e3fe136529b2ab816ee79ca480f6242d89ed505a0
Windows binary: abbd38115cff5b05dc383a042d038144485e34950f2165f935634ed2c5d85ebe
Linux tar.gz:   8ff255568d815e83428b0612bc1bcc01d63d629d3e68797422d541e141547c36
Windows zip:    0b37438c050770c01fd6b32a994857f4df51e0eca5b6b30cd097668473d4744a
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
| Desktop/terminal adapter | pending |
| Real-provider quality | unverified, no credentials |

## Known risks

- Requests beyond the concurrency limit wait inside the process rather than receiving queue-full.
- There is no per-request cancellation control beyond global Ctrl+C.
- Stdin/stdout transport is local-only; authentication belongs to future transports.
- Provider credentials are inherited from the host environment.

## Next requirement

Add per-request cancellation and queue admission:

1. Add request ids to cancellation controls.
2. Admit up to a configured number of simultaneous runs.
3. Return structured queue-full and cancelled responses.
4. Keep journal writes independent and run-scoped.
5. Test cancellation and two concurrent requests against a deterministic slow provider.
