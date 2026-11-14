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
- `history` CLI command for recent run discovery from journal tails.
- Host request ids are persisted as `ClientRequestLinked` journal events.
- History discovery restores client request ids and stop reasons from bounded head/tail reads.
- Terminal host status includes the authoritative journal path.
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
| Unit tests | 44 passed |
| Real subprocess host request | passed |
| Invalid JSON followed by a valid request | passed; host survives and continues |
| Two concurrent requests plus invalid input | passed; all 3 responses correlated correctly |
| Queue full plus cancel by request id | passed; target cancelled, peer unaffected |
| Provider inspection with secret environment | passed; env name shown, value absent |
| Host registry active/terminal state | passed |
| Journal-tail history discovery | passed |
| Host request-id to journal binding | passed |
| Host result semantic audit | passed |
| Existing CLI/config/experience/resume/eval suites | passed |
| Clippy `-D warnings` | passed |

Release artifacts:

```text
Linux binary:   6b542b4b5a3d052afac32382bb8e55015005e4d8fce339ad09bb7ec082e54c07
Windows binary: d186cb15dc07ad7365bb0d6529ce645732c48d02103ebf4f7067da16b6bdf0d3
Linux tar.gz:   b4e238801a4e33de928eb5bdddd461dc386488cdd3c37931933bd2a62199fd35
Windows zip:    6ff584f8b620e7236126ca3f6632cf019c1e81cda98f391ed167ea950bfd795b
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
| Run-history discovery | implemented with bounded tail reads and 200-entry output cap |
| Request-id to journal binding | implemented and restart-discoverable |
| Desktop/terminal adapter | pending |
| Real-provider quality | unverified, no credentials |

## Known risks

- Stdin/stdout transport is local-only; authentication belongs to future transports.
- Provider credentials are inherited from the host environment.

## Next requirement

Add held-out experience transfer evaluation:

1. Split learning and evaluation task groups.
2. Activate candidates from group A only.
3. Compare baseline and learned behavior on unseen group B.
4. Reject candidates with negative transfer or unsafe confidence increases.
5. Persist the evaluation evidence with the candidate.
