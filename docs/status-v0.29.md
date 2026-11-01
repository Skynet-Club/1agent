# Status v0.29.0

Date: 2026-11-02
Status: inert tool conformance implemented

## Changes since v0.28.0

- Added `EchoTool` with strict `{ "text": string }` arguments.
- Echo text is limited to 256 characters and rejects hidden/bidirectional controls.
- Echo has no filesystem, process, environment, write or network authority.
- Added `providers --probe-tools --allow-network`.
- Tool probe requires exactly:
  1. one tool call;
  2. non-empty call ID;
  3. tool name `echo`;
  4. JSON arguments accepted by deterministic validation;
  5. successful observation;
  6. a second model call producing a valid final `AgentAnswer`.
- Reports tool-call, observation and final-answer booleans plus latency/tokens/error code.
- Raw tool payloads and remote responses are not persisted in the report.

## Verification

| Check | Result |
|---|---|
| Inert echo observation | passed |
| Hidden control rejection | passed |
| Tool call/observation/final round trip | passed with scripted provider |
| No tools offered without registration | enforced |
| Unit tests | 72 passed |
| Core evaluation rubric | 4/4 passed, 0 hard-gate failures |
| Clean-source package gate | passed |

## Verified clean release

```text
source commit: 8b51479c1656365fed7da4fe92cf2cc8201c8935
git dirty: false

Linux binary:
cc2b487edd1c1fa97ae4f6cc4d13ef13642c88d665ec2cf713e587bd6532611a

Windows binary:
a050cdb9c2d1b5a4d00a236e245048e23c4429a5a02368506c833ba8e1f24712

Linux tar.gz:
c5460d69e2f2a21fdb6b0a61eebd0f1fc33436dd89b473accc4e16bdcabce4e6

Windows zip:
df3578ae7d4753f474d881b66a6a3f480b09e2e33bae6240d7d90d2581a1bdba

Release manifest:
60c14457ba435a1e96a174aaa16179496dfed9536fa40441ee810e36db088c08
```

## Known limitations

- Successful live tool calling cannot run because the managed sandbox blocks loopback binding
  and no external model credentials are available.
- A scripted provider proves transport-neutral orchestration, not a specific vendor's behavior.
- The probe allows only one tool invocation; multi-call ordering remains separate work.
- Tool argument schemas are validated by deterministic code but model schema adherence is not
  guaranteed across providers.

## Next requirement

Evaluate route selection under explicit objectives:

1. Add quality/latency/cost route-policy modes.
2. Keep deterministic capability and privacy filters as hard constraints.
3. Record which objective selected each route.
4. Test ties and no-route cases.
5. Prevent learned experience from overriding hard route constraints.
