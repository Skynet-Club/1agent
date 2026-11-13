# agent-loop-v1 100-question run

- Date: 2026-11-13
- Suite: `agent-loop-v1` v1.3
- Suite SHA-256: `c5beab60a90fc55fdcea5b5f2d4356cb7fe6025836eb056d6c23877e36cf2ea6`
- Provider: `deepseek-primary` / `deepseek-flash` via `configs/pi-deepseek.json`
- Command:

```bash
PI_DEEPSEEK_API_KEY=... 1agent eval --suite evals/agent-loop-v1.json \
  --config configs/pi-deepseek.json --allow-network \
  --artifacts-dir docs/evidence/agent-loop-v1-2026-11-13 --json
```

- Result: **96/100 passed, 4 failed**
- Tokens: 1365416; cost: 2881390 micros
- Report SHA-256: `9924feac03f2d33d110f85b3a63073707cdbe312e8cde49ade5c4219e8116061`

## Artifacts

- `cases.jsonl`: one record per case, including the persistent journal path.
- `journals/<case-id>/<run-id>.jsonl`: raw run journals, including the
  fast-chat run and the full-agent run when the case escalated.

## Known limits

- One provider/model only; this is a model-quality suite, not a deterministic
  contract fixture.
- A few open-ended current-info questions can exhaust the 8-tool budget and
  return `needs_evidence`; those are reported as failures, not hidden.
- Human review of the 100 answers is still pending. A green run is not a
  release announcement.

## Raw journals

The live journal tree is about 5.9 MB and includes third-party web text
retrieved by the tools. It stays in the ignored `journals/` directory so that
retrieved content does not enter the product tree; `journals.sha256` records
every journal path and digest, so the tree can be verified without committing
the content. The full `journals/` directory is archived with the release
evidence package.
