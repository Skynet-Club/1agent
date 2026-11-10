# Web fetch evidence — 2026-11-09

Provider: `openai-compatible` / `deepseek-flash`  
Host: Linux x86_64  
Command shape: `1agent run --task "<objective>" --json`

## Public HTTPS fetch

Objective: use `fetch_url` to read `https://example.com` and summarize it.

Observed tool call:

```text
fetch_url
```

Result: `stop_reason=completed`; the tool returned HTTP 200 and the extracted
text identified the page as the IANA `Example Domain` placeholder. The model did
not use `curl` or the terminal.

## Public search

Objective: use `web_search` for `Rust programming language` and report the
first three results.

Observed tool call:

```text
web_search
```

Result: `stop_reason=completed`; the tool returned 8 results. The first three
were the Rust language site, a Rust tutorial and the Rust install page. Bing RSS
redirected to the localized `cn.bing` market, so results can be localized.

## Artifacts

| File | SHA-256 |
|---|---|
| `fetch-example-run.jsonl` | `1c1c6733551e83f100bcc9a5ea2eb1cf0d088ed46d7901c175133ede2a74335c` |
| `search-rust-run.jsonl` | `e95d6920a1fb49d426db7c02452b395a71d607a0a00e50605956fc5a7c8ea064` |

## Caveats

- One public URL; no search provider and no authenticated web content.
- Private/loopback/link-local blocking, HTTPS-only, redirect limits and response
  size limits are covered by deterministic unit tests in `src/tools/web.rs`.
- The live test requires `--allow-network`; the `1agent` wrapper supplies it for
  `run`/`tui`.
