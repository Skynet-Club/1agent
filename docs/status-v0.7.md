# Status v0.7.0

Date: 2026-11-01
Status: versioned offline evaluation harness

## Added since v0.6.0

- Versioned JSON evaluation suites.
- Isolated temporary workspace per case with safe relative setup files.
- Expected stop reason, answer status, minimum evidence, minimum claims and audit result.
- Hard-gate accounting for audit failures and execution errors.
- JSON report and human-readable CLI scorecard.
- `1agent eval --suite evals/core.json`.

## Core suite

`evals/core.json` contains:

| Case | Expected behavior |
|---|---|
| `read-evidence` | read-only evidence path completes with evidence and a verified claim |
| `needs-authorization` | destructive request stops before provider/tool execution |
| `honest-without-reasoning-provider` | deterministic provider returns `needs_evidence` rather than guessing |
| `hidden-control-rejected` | poisoned evidence is rejected with `unsafe_content`, then reported honestly |

Current result:

```text
评测：core-v1，通过 4/4，硬门禁失败 0
```

Release artifacts:

```text
Linux:   8.1 MiB
sha256: c3c76f91c24f01bf253da3fe0d31c7b9381c52f1a202af232e83c627e364cf2f

Windows: 12 MiB, PE32+ x86-64
sha256: 4db214dfb3506ceae95de4abc0a65350588740b91d8cb9e736f53dd000916853

Archives:

Linux tar.gz:
sha256: 7a8d007e0840223366745cf459d1aa85e404ae21c88e49993f8436715fc4823e

Windows zip:
sha256: 2e629b681b7d74253a83216cb0d9f8b613083d200a95d7dfdb6bc87295433858
```

## Verification

| Check | Result |
|---|---|
| Unit tests | 39 passed |
| Core evaluation suite | 4/4 passed |
| CLI e2e/config/experience/resume | passed |
| Clippy `-D warnings` | passed |
| Linux release build | passed |
| Windows GNU release build | passed; PE32+ x86-64 artifact produced |
| Search result limit/truncation | passed |
| Tool timeout as structured failure | passed |

## Requirement comparison

| Requirement | Status |
|---|---|
| Versioned cases | implemented |
| Isolated workspaces | implemented |
| Hard gates | implemented |
| JSON/human report | implemented |
| Real-provider opt-in suite | configuration implemented; live execution blocked by missing credentials |
| Human gold quality scoring | pending |
| Provider/model/latency/usage metrics | implemented in case reports |

## Known risks

- The core suite uses the deterministic provider; it cannot establish real-model quality.
- Evidence and claim counts are structural metrics, not semantic correctness.
- A malicious evaluation file can still create many files or very long content inside the temp root.
- No parallel case execution or per-case timeout is implemented yet.

## Next requirement

Complete live-provider evaluation and richer hard-gate cases:

1. Separate deterministic contract suite from real-provider quality suite.
2. Require explicit network and model configuration for live evaluation.
3. Add source-conflict, tool-timeout, prompt-injection and checkpoint cases.
4. Record provider, model, latency and usage per case.
5. Preserve deterministic results as the release gate.

Current blockers:

- no DeepSeek, Kimi, Qwen, OpenAI or other provider credential is present;
- no local Ollama service is installed/running;
- real-model quality and provider costs therefore remain unverified.
