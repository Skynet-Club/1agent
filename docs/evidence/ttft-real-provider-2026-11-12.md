# Real-Provider TTFT — Evidence (2026-11-12)

Status: **local baseline (n=3)**
Provider: DeepSeek `deepseek-flash` via `configs/pi-deepseek.json`
Tasks: three one-step direct replies (`alpha`, `beta`, `gamma`), `--stream`

## Command

```bash
export PI_DEEPSEEK_API_KEY="$(jq -er '.deepseek.key' ~/.pi/agent/auth.json)"
for task in alpha beta gamma; do
  1agent run --task "Reply with exactly: $task" \
    --config configs/pi-deepseek.json --allow-network --stream \
    --journal-dir /tmp/1a-ttft --timeout-seconds 120 --max-steps 1
done
1agent metrics --journal-dir /tmp/1a-ttft --json
```

## Results

`StreamFirstToken.elapsed_ms` per run:

```text
deepseek-primary  deepseek-flash  719
deepseek-primary  deepseek-flash  2480
deepseek-primary  deepseek-flash  1079
```

`1agent metrics`:

```json
{
  "runs": 3,
  "completed": 3,
  "ttft_latency": {
    "count": 3,
    "min_ms": 719,
    "p50_ms": 1079,
    "p95_ms": 2480,
    "max_ms": 2480
  }
}
```

Raw files:

```text
docs/evidence/ttft-real-provider-2026-11-12/events.tsv
docs/evidence/ttft-real-provider-2026-11-12/metrics.json
```

## What this proves

- Real streaming runs emit `StreamFirstToken` and `metrics` aggregates TTFT.
- On this machine/provider, p50 is 1.08 s and p95 is 2.48 s for three short
  direct tasks.

## What this does not prove

- A statistically useful TTFT distribution: three samples, one provider/model,
  and only short direct prompts.
- Long-context, tool-dense or retry-heavy TTFT; those are not sampled.
- Cost per run: provider pricing is not embedded, `cost_micros` stays 0.
