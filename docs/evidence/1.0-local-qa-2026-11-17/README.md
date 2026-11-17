# 1.0 local QA evidence (2026-11-17)

Scope: code-level 1.0 local gates. External audit, real Windows/macOS GA,
marketplace and R1–R9 user metrics remain blocked.

## 1. Hard gate

`./scripts/verify.sh`: **pass**, `verification passed`.

| Metric | Value |
|---|---:|
| Library tests | 1196 passed / 0 failed / 4 ignored |
| All test targets | 1637 passed / 0 failed / 13 ignored |
| core-v3 | 8/8, hard-gate failures 0, rubric 6/6 |
| claims structure | well-formed |

Raw log: `verify.log`.

## 2. HTTP surface e2e

`cargo test --offline --test http_surface_e2e -- --ignored`: **1 passed /
0 failed**. Raw log: `http-e2e.log`.

## 3. TUI interaction

`./scripts/tui-interaction-check.sh`: **pass** (real provider config).
Covered: greeting completes without the next-action modal, `/help`, `/status`,
`/mode`, Ctrl+T, categorized drawer, Tab completion, destructive confirmation,
panel focus isolation, Backspace safety, one Ctrl+Q only arms quit, Esc
cancels, two Ctrl+Q exits, and 400 mixed paste/space/backspace edit events.

Two harness fixes were required:

- `scripts/pty_screenshot.py` now sends two Ctrl+Q presses (the 0.99 quit
  contract) instead of Ctrl+Q + Enter;
- `scripts/tui-interaction-check.sh` allows 12s for the live greeting and 45s
  outer timeout; `scripts/pty_backspace.py` tests one-press arm + Esc cancel +
  two-press exit.

Raw log: `tui-interaction.log`.

## 4. Soak

`cargo test --offline --test a_gate_soak -- --ignored -q` × 5: **all pass**.
Raw log: `soak.log`.

## 5. Python SDK

`python3 -m unittest discover -s sdk/python`: **6/6 pass**, including
`/v0/openapi`. Raw log: `sdk-python.log`.

## 6. Release-check

`./scripts/release-check.sh`: **fmt, clippy, tests, HTTP e2e pass**; `ci.sh`
then blocks at `scripts/check-attribution.sh` because the development history
contains an AI byline marker and pre-existing product-comparison docs contain
vendor references. This is a release-hygiene blocker, not a code-test failure.
A public release must use a clean release branch or an amended attribution
policy; do not rewrite the development branch silently.

The script now keeps `cargo` tests hermetic via an isolated `ONEAGENT_HOME`
while running the TUI check against the developer's real provider config.

Raw log: `release-check.log`.

## 7. Track Q final

`agent-loop-v1`: **99/100**, 0 hard-gate failures, audit/replay 100/100,
rubric 20/20. Residual `chat-19` used 2 provider calls where 1 is allowed for a
simple chat turn. Evidence:
`docs/evidence/0.99-acceptance/agent-loop-v1-final-2026-11-17/`.

## 8. rc.2 permission UX and re-run (2026-11-18)

The rc.2 line fixes the TUI permission UX bug: the model offered “开网络权限”
as a clarify option but could not grant it. The model prompt, clarify tool
description, `describe_self` limitations and every network-denied tool error
now state that the human enables access for the next run with
`/permissions network on` (or `--allow-network` headless); no TUI restart is
required. No auto-elevation was added.

Verification:

| Check | Result |
|---|---|
| `./scripts/verify.sh` | **pass**; 1638 total, 0 failed, 13 ignored |
| Prompt/self-model tests | pass (`system_prompt_does_not_let_the_model_grant_permissions`, `self_model_reports_only_available_tools_and_live_budgets`) |
| `agent-loop-v1` rc.2 | **99/100**, 0 hard-gate, audit/replay 100/100, rubric 20/20 |
| Residual | `intro-20` used 6 tool calls where 4 are allowed (three `describe_self` calls); efficiency only, no safety gate |

`chat-19`, the rc.1 residual, passed in this run; both runs are one
provider/model sample, so the residual moved rather than disappeared. Evidence:
`docs/evidence/1.0-local-qa-2026-11-17/agent-loop-v1-rc2-2026-11-18/` and
`verify-1.0.0-rc.2.log`.

## 9. rc.3 full-power defaults (2026-11-18)

Local TUI and `run` now enable writes, terminal execution and network by
default. `--no-network` and `--read-only` are the explicit opt-outs; the TUI
still applies `/permissions network off|on` to the next run only.

The TUI previously ignored the configured permission preset. A `full-local`
config with a stale `allow_network: false` field stayed offline. The TUI now
resolves the preset through `AgentConfig::resolved_permissions`, so
`full-local` means network on. New installs default to `full-local`; explicit
`safe`/`read-only` configs keep their gate. `eval`/`benchmark-run` keep their
explicit `--allow-network` flag so benchmark inputs stay reproducible.

| Check | Result |
|---|---|
| `./scripts/verify.sh` | **pass**; 1640 total, 0 failed, 13 ignored |
| claims table | well-formed; 258 checked (234 verified / 16 partial / 8 no evidence) |
| TUI interaction | **pass** (real provider; two-Ctrl+Q quit) |
| Default/opt-out e2e | `tui_network_permission_defaults_on_and_no_network_disables_it` pass, including stale `full-local` + `allow_network:false` |

Security note: default network widens the prompt-injection egress surface;
O1 (`browser_read` SSRF) and O2 (journal hash chain) remain open. Evidence:
`verify-1.0.0-rc.3.log`, `tui-interaction-rc.3.log`.

## 10. rc.4 right-click paste fix (2026-11-18)

The TUI composer had a right-click binding that read the system clipboard and
inserted it. Because clipboard content survives a TUI restart, a stale answer
was reinserted on every right-click. The binding is removed; paste is explicit
with `Ctrl+V`/`Shift+Insert`.

Verification: `verify.sh` **1640 total / 0 failed / 13 ignored**. A PTY test
sets a fake `wl-paste` marker, sends an SGR right-click in the composer, and
checks the screen: the marker is absent after right-click and present after
Ctrl+V. Raw logs: `verify-1.0.0-rc.4.log`, `right-click-rc.4.log`.

## 11. rc.5 settings and themes (2026-11-18)

`/setting` is the unified TUI settings entry (`/settings` alias; `/config`
compatibility). It shows effective values and source, sets budgets, language,
network/write and theme, and offers two reset levels. `/setting reset global`
warns first, then requires `confirm`, backs up `config.json`, restores
full-local/network/write and military green, and preserves providers, keys,
workspace and read roots.

Eight TUI-only themes are implemented as a post-render remap of the semantic
base palette: military (default), matrix, mono, ocean, sakura, copper, cyber
and wasteland. The theme is stored in `config.json` `appearance.theme`
(schema 3; v1/v2 configs still load). CLI help is unchanged.

| Check | Result |
|---|---|
| `./scripts/verify.sh` | **pass**; 1648 total, 0 failed, 13 ignored |
| TUI interaction | **pass** |
| Settings e2e | `/setting show`, `/settings theme matrix`, theme list pass |
| Theme tests | 8 unique themes; military identity; matrix remap |
| Reset tests | session restores startup; global backups + restores military/full-local |

Raw logs: `verify-1.0.0-rc.5.log`, `tui-interaction-rc.5.log`.

## 12. Not covered

- 8h DeepSeek model-quality benchmark (cost/time not approved; prior 8h local
  soak is runtime-only evidence).
- External A-Gate/W-Gate, third-party audit, real Windows/macOS hosts, signing,
  notarization, marketplace, mobile, R1–R9.
- `check-attribution.sh` on the development history.
