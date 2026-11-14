# Real-Provider TTFT — Evidence (2026-11-13)

Status: **local sample (n=20), one provider**

Provider: DeepSeek `deepseek-primary` / `deepseek-flash` via
`configs/pi-deepseek.json`
Tasks: 10 short direct replies and 10 long-context prompts (~27k characters,
about 7k input tokens), all with `--stream --max-steps 2` and an explicit
“Do not call any tools” instruction.

## Command

```bash
~/.cache/1agent-soak/ttft-samples-2026-11-13.sh
```

The wrapper runs each task with `--stream`, then runs
`1agent metrics --journal-dir <dir> --json`, extracts the first
`StreamFirstToken.elapsed_ms` per journal and writes `events.tsv` and
`summary.json`.

## Results

```text
                 count  min    p50    p95    max
short direct       10   848   1239   1942   1942
long context       10   979   1573   2786   2786
overall            20   848   1498   2768   2786
```

Earlier n=3 short baseline: p50 1.08 s / p95 2.48 s. The larger sample has a
higher p50 and p95; the three-sample baseline was not statistically stable.

Raw files:

- `docs/evidence/ttft-real-provider-2026-11-13/events.tsv`
  (sha256 `2f6143e49583ae37bc6efb5ec79cd06f9abdb0a9ff300e0d653e7966b18c3f51`)
- `docs/evidence/ttft-real-provider-2026-11-13/metrics.json`
  (sha256 `8ae9fb746e02bc418270ae7ee86f7227d9e921678aa8a86b8192403c5400bff8`)
- `docs/evidence/ttft-real-provider-2026-11-13/summary.json`
  (sha256 `2701caddfe84ae98c52b2139466f2939c530697f4799edea59f54dce5da6ecb6`)

## What this proves

- Real streaming runs emit `StreamFirstToken`; `metrics` aggregates TTFT.
- On this machine/provider, 20 mixed short/long tasks have p50 1.50 s and p95
  2.77 s; long-context prompts are slower than short direct prompts.

## What this does not prove

- A second provider or model; only DeepSeek `deepseek-flash`.
- Tool-dense or retry-heavy TTFT; tools were disabled for this sample.
- Production latency or a statistically sufficient distribution; n=20.
- Cost per run: provider pricing is not embedded.
