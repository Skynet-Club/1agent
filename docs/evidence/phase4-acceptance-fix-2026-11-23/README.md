# Phase 4 acceptance fix — 2026-11-23

Status: **targeted fix verified on the neutral three-arm benchmark; the product
pass^3 did not improve and the remaining blocker moved to answer repair.**

Binary: `/home/skynet/.local/bin/1agent` sha256
`d20468cd8e7522ff6abf9780c7aaba20e6aa1b3ff5b2bef9bea6b0df4bbe50bb`.
Previous binary backup: `~/.local/bin/1agent-pre-acceptance-fix-2026-11-23`
(sha256 `003e33191fb5…`).

## Change

- `src/tools/terminal.rs`: conservative read-only shell classifier. A command
  is read-only only when every segment starts with a known read-only command and
  there is no redirection/substitution; unknown syntax counts as a write.
  `terminal_exec` now records `write=true|false` in its observation metadata.
- `src/runtime.rs`: the completion gate ignores `terminal_create`,
  `terminal_output`, `terminal_destroy`, and a `terminal_exec` whose metadata
  says `write=false`. A read-only `jq`/`ls`/`sha256sum` after acceptance no
  longer clears the receipt or forces a new acceptance call.
- `src/tools/acceptance.rs`: verifier labels without a recognized prefix are
  normalized to `verifier:…` when independent verification is required. Human
  verification still requires the explicit `human:` prefix.
- `src/verification.rs`: criterion matching ignores case and extra whitespace.
- `src/provider.rs`: prompt now says a later accepted receipt is decisive
  (earlier failed attempts do not force conditional), every public and hidden
  criterion needs a matching result, and read-only inspection should prefer
  `read_file`/`jq`/`rg`/`ls` over interpreters.

Tests: `command_write_detection_is_conservative`,
`read_only_terminal_exec_is_not_a_completion_side_effect`,
`independent_verifier_label_is_normalized`,
`criterion_matching_ignores_case_and_whitespace`.

## Neutral three-arm benchmark

Same protocol as the baseline in `../phase4-2026-11-23/`.

| Arm | Neutral completion | 1agent status met | Model calls (median) | Tool calls | Wall ms | TTFT ms | Fixed tokens | Prompt tokens | Output tokens |
|---|---|---|---|---|---|---|---|---|---|
| full | 24/24 | 17/24 (baseline 16) | 6.0 | 5.0 | 17,553 (baseline 16,413) | 882.5 | 8,971 | 70,537 | 2,899.5 |
| core7 | 24/24 | 21/24 (baseline 18) | 5.0 (baseline 6) | 3.0 (baseline 5) | 16,522 (baseline 17,173) | 867 | 4,476.5 | 24,965 (baseline 33,164) | 2,837 |
| pi | 24/24 | — | 3.5 | 3.0 | 5,434 | 522 | 597 | 6,836.5 | 541.5 |

Core 7 is the clear beneficiary: status met +3, one fewer model call, two fewer
tool calls, prompt tokens -25%, wall time -4%. Full improved only +1 and its
median wall time rose 7%.

## Product eval N=3

| Run | Overall pass | pass^3 | pass at least once | Failures |
|---|---|---|---|---|
| Baseline (`003e3319…`) | 20/24 = 83.33% | 6/8 = 75% | 8/8 = 100% | long-task 1/3, checksum 1/3 |
| Acceptance fix (`d20468cd…`) | 19/24 = 79.16% | 3/8 = 37.5% | 8/8 = 100% | bug-fix 2/3, config 2/3, data 2/3, long-task 2/3, checksum 2/3 |

Failure classes after the fix: `evidence` 4, `generation` 1, `infra` 1,
`unknown` 1. This is a **small-N product regression signal**, not a proven
causal regression: failures spread across five cases with one failure each,
while the acceptance-specific failures fell.

The remaining failures are answer generation/repair, not acceptance:

- final answer with 0 claims/evidence (`data-processing-sales`,
  `long-task-plan`, `checksum-file`, `bug-fix`);
- `invalid_final` with trailing text after the JSON object (`config-retries`);
- one provider `InvalidResponse` ("tool call arguments are invalid JSON")
  (`core7` config-retries run 3 in the neutral benchmark);
- a model answer that self-downgraded to `conditional` after a failed
  acceptance attempt even when a later attempt was accepted (`config-retries`).

## Gates

- `cargo fmt --check`
- strict clippy `--all-targets -D warnings`
- lib tests: 1435 passed / 0 failed / 4 ignored
- bin tests: 69 passed
- core eval: 8/8
- `scripts/verify.sh` stops at the pre-existing
  `cli_tui_agents_refresh_uses_configured_profiles` integration test; it fails
  identically on a clean `8e1dfc2` worktree in this environment. The other
  39 `cli_e2e` tests pass.
- Installed binary verified: sha256 `d20468cd…`; previous binary backed up.
- A running TUI keeps the old inode until restarted.

## Next step

The acceptance gate is no longer the main blocker. The next slice should be
**answer repair and validation**:

1. Accept a single JSON object followed by trailing prose (the observed
   `invalid_final` case), instead of failing with `Extra data`.
2. Make the repair contract return claims and evidence for `completed`, or fail
   with an actionable missing field; do not end at 0 claims/0 evidence.
3. Treat `InvalidResponse` from a malformed tool call as retryable within the
   provider-attempt budget.
4. Consider a narrow runtime upgrade from `conditional` to `completed` only
   when an accepted receipt exists and the sole blocker is an earlier
   acceptance attempt; do not relax other conditional reasons.
5. Based on the neutral result, evaluate Core 7 as the default Act surface.

## Limits

- N=3 and eight tasks: directional only; the pass^3 difference is noisy.
- The product eval runs the full arm, not Core 7; the neutral benchmark is the
  cross-arm evidence.
- Neutral checks judge results, not process quality.
- Pi runs in Docker; 1agent runs on the host.
- No human blind review; user corrections are not measured.
