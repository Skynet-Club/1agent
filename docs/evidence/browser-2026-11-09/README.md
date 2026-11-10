# Headless browser evidence — 2026-11-09

Provider: `openai-compatible` / `deepseek-flash`  
Host: Linux x86_64; Google Chrome installed  
Command shape: `1agent run --task "<objective>" --json`

## Rendered read

Objective: use `browser_read` to render `https://example.com` and summarize it.
The model did not use `fetch_url` or `curl`.

Observed tool calls:

```text
browser_read -> skill_list -> browser_read -> browser_read
```

The first `browser_read` returned 927 extracted characters; the page was the
IANA `Example Domain` placeholder. The run finished with `stop_reason=completed`.

## Artifact

| File | SHA-256 |
|---|---|
| `render-example-run.jsonl` | `b8b03d19137484c6c29cb897496ffdd57bdc16f4291534d02952f1de316a4de9` |

## Caveats

- Read-only rendering only: no click, typing, screenshots or computer-use.
- URL must be public HTTPS; the same public-address checks as `fetch_url` run
  before Chrome starts. Chrome resolves independently, so DNS rebinding is a
  residual risk until a pinned resolver is added.
- Fake-Chrome tests cover network permission, HTTP rejection and DOM extraction;
  the live test covers one public page.
