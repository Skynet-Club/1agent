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
| Clean-source package gate | passed |

## Known limitations

- Finding messages remain diagnostic prose; clients should branch on `code`.
- Codes in this release cover current journal invariants, not future policy violations.
- The audit report itself is not signed.

## Verified clean release

```text
source commit: 6ea123815a4096147f4b3e53f933aec0f1981e3b
git dirty: false

Linux binary:
45b459169ad5d31884f36038d33e60ce988f284ffb21b8eda93cbc24bd29e1b8

Windows binary:
985677874822287cf88f6f10276ab060fe0836203e08c4870d3f3d8cc35698cd

Linux tar.gz:
101e49f96d986203fc981b115e471037bbd6c8dc17f5e9f8205bce25cb7a3be2

Windows zip:
74028194f9d9a023139c4586d09cc30a1d34c33b1937c8f64d4ef0019feed63a

Release manifest:
f626e6340935cf5e7dd354e2692852ce914ee4c6e0a9e3141f2c12d674499f31
```

## Next requirement

Bound untrusted remote provider responses before JSON decoding:

1. enforce a maximum response body size;
2. treat oversized responses as structured provider errors;
3. keep complete bodies out of journals and errors;
4. test chunked and declared-length responses;
5. document remaining streaming and timeout limits.
