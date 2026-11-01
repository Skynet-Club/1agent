# Status v0.55.0

Date: 2026-11-02
Status: structured audit findings implemented

## Changes since v0.54.0

- Journal audit JSON adds structured `findings`.
- Each finding has:
  - a stable snake-case `code`;
  - an optional `event_index`;
  - the original human-readable message.
- Existing `issues` strings remain present for backward compatibility.
- Unpaired tool findings point to the original tool-call event.
- Answer validation findings point to the answer event.
- Route, sequence, duplicate, stop and missing-answer findings carry the relevant event index
  where available.

## Stable finding codes

```text
unsupported_protocol_version
non_sequential_event
run_id_changed
duplicate_run_start
duplicate_tool_call
duplicate_tool_observation
route_selection_outside_plan
duplicate_run_stop
duplicate_answer
empty_journal
first_event_not_run_started
unpaired_tool_call
missing_run_stop
answer_validation_failed
stop_reason_mismatch
missing_answer_for_stop
```

## Verification

| Check | Result |
|---|---|
| Unit tests | 95 passed |
| Integration / E2E | passed |
| Core evaluation | 6/6 passed |
| Duplicate answer code and index | passed |
| Unpaired tool code and index | passed |
| Answer validation code and index | passed |
| Serialized snake-case code | passed |
| Legacy issues compatibility | passed |
| Clippy `-D warnings` | passed |
| Clean-source package gate | pending |

## Known limitations

- Finding messages remain diagnostic prose; clients should branch on `code`.
- Codes in this release cover current journal invariants, not future policy violations.
- The audit report itself is not signed.

## Next requirement

Bound untrusted remote provider responses before JSON decoding:

1. enforce a maximum response body size;
2. treat oversized responses as structured provider errors;
3. keep complete bodies out of journals and errors;
4. test chunked and declared-length responses;
5. document remaining streaming and timeout limits.
