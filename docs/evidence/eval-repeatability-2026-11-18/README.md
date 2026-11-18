# Repeated evaluation smoke (2026-11-18)

Scope: validate the new repeat/pass^k and process-metric path on a small
real-model subset. This is **not** a full Track Q stability claim.

## Command

```bash
1agent eval --suite suite.json --config ~/.1a/config.json \
  --repeats 2 --artifacts-dir /tmp/agent-loop-repeat-smoke-art --json
```

## Result

- Suite: `agent-loop-repeat-smoke` (4 cases: `intro-03`, `intro-20`,
  `chat-19`, `risk-12`)
- Repeats: 2
- Attempts: 8; passed: 7
- Overall pass rate: 87.50%
- `pass^2`: 75.00%
- At least once: 100%
- Failure class: `budget` × 1

`chat-19` is the demonstration: one attempt used 2 provider calls (limit 1) and
failed; the other used 1 and passed. This is exactly the instability that a
single 99/100 run hides. `intro-20` passed both attempts in this smoke;
`risk-12` passed both.

Raw:
- `repeat-report.json`
- `chat-19-attempts.json`
- `stdout.json`
- `suite.json`

## Limits

- Four cases × two attempts is a smoke test, not a stability claim for the
  full 100-case suite.
- No human calibration set yet; failure classes are rule-derived.
- 8h DeepSeek quality and R1–R9 remain blocked.
