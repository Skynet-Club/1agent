# 0.98 Setup Wizard — PTY End-to-End Evidence

Date: 2026-11-15  
Branch: `0.98-dev` (`12bf5ea` plus the uncommitted wizard change)  
Binary: `target/debug/first-principles-agent` (`0.98.0-dev`)  
Terminal: PTY, 100×32 by default, `TERM=xterm-256color`  
Provider: local fake OpenAI-compatible server (`tools/fake_provider.py`), so
the run is deterministic and offline.

## Why this evidence exists

The setup wizard is the first user-visible surface of 0.98. The automated
tests cover layout and contracts, but the arrow-key flow, alternate-screen
cleanup, mask rendering, probe ordering and idempotency only exist on a real
TTY. This evidence drives the actual binary through a PTY and records the
screens and file outcomes.

## Scenarios

| Scenario | Input path | Result |
|---|---|---|
| Fresh OpenAI-compatible | custom loopback endpoint, model menu, safe, current workspace | setup ready; smoke `OK`; exit 0 |
| Existing config with an unadvertised model | existing `legacy-model`, probe returns two other ids | `legacy-model` shown with `not advertised`; selected `fake-model-a`; config updated |
| Idempotent rerun | same choices again | `config.json` SHA-256 unchanged |
| Masked key | non-loopback endpoint, `--skip-probe --skip-smoke` | key file `0600`, content matches, key absent from all output; status ready |
| Probe failure → manual model | unreachable loopback endpoint, `--skip-smoke` | Retry/Manual menu; `manual-model` written; setup does not repeat the failed probe; exit 0 |
| Cancel | `Ctrl-C` at welcome | exit 130, no config, alternate screen restored |
| Narrow terminal | 40×20 PTY | compact `1 · A G E N T` wordmark, no overflow |
| Color palette | default color mode | ANSI RGB `#3fb950`, `#4a7855`, `#5e9e5e` present |
| Release binary | `target/release/first-principles-agent` (the installed `1agent`) | fresh and existing-config flows exit 0; all 11 Ready-panel lines are 80 columns |

## Reproduce

```bash
cargo build --offline --bin first-principles-agent

D=/tmp/1agent-wizard-e2e
mkdir -p "$D/root" "$D/workspace"
python3 -I tools/fake_provider.py 18080 "$D/requests.log" &
python3 -I tools/drive_wizard.py \
  target/debug/first-principles-agent "$D/root" "$D/workspace" \
  "$D/wizard.raw" "$D/wizard.plain" 18080 fresh
```

`tools/drive_wizard.py` accepts a scenario name as its last argument:
`fresh`, `existing`, `existing_same`, `key`, `probe_fail`, `cancel`.

For the existing-model scenario, create a config first:

```bash
target/debug/first-principles-agent setup --non-interactive \
  --root "$D/root" --provider openai-compatible \
  --base-url http://127.0.0.1:18080/v1 --model fake-model-a \
  --permission-preset safe --workspace "$D/workspace" \
  --skip-probe --skip-smoke --json >/dev/null
jq '.providers[0].model="legacy-model"' "$D/root/config.json" > "$D/root/config.tmp"
mv "$D/root/config.tmp" "$D/root/config.json"
python3 -I tools/drive_wizard.py ... "$D/root" "$D/workspace" ... 18080 existing
```

## Captured screens

Fresh welcome (plain capture, color stripped; the real render uses the
military-green palette):

```text
  ────────────────────────────────────────────────────────────
                     ██╗ █████╗  ██████╗ ███████╗███╗   ██╗████████╗
                    ███║██╔══██╗██╔════╝ ██╔════╝████╗  ██║╚══██╔══╝
                    ╚██║███████║██║  ███╗█████╗  ██╔██╗ ██║   ██║
                     ██║██╔══██║██║   ██║██╔══╝  ██║╚██╗██║   ██║
                     ██║██║  ██║╚██████╔╝███████╗██║ ╚████║   ██║
                     ╚═╝╚═╝  ╚═╝ ╚═════╝ ╚══════╝╚═╝  ╚═══╝   ╚═╝
                  EVIDENCE-ACCOUNTABLE AGENT RUNTIME
                        v0.98.0-dev · REAL WORLD
                Welcome. Let's get your first task running.
                     •  Pick a provider and model
                  •  Choose permissions and a workspace
                 •  Verify everything with a short smoke run
                   Press Enter to begin   Ctrl-C to quit
  ────────────────────────────────────────────────────────────
```

Unadvertised current model (yellow badge in the real render):

```text
  Choose a model
  Models come from the provider's /v1/models list.

  ❯  1  legacy-model             configured model  · not advertised
     2  fake-model-a
     3  fake-model-b
     4  Enter another model id…  use the exact provider id
```

Review before any write:

```text
  ┌─ Review ───────────────────────────────────────────────────────────────┐
  │ Provider     OpenAI-compatible (openai-compatible)                     │
  │ Endpoint     http://127.0.0.1:18081/v1                                 │
  │ Model        fake-model-a                                              │
  │ Credential   not required                                              │
  │ Permissions  safe · workspace writes · network/exec need approval      │
  │ Workspace    ~/.claude/jobs/.../workspace-b                            │
  │ Runtime home ~/.claude/jobs/.../root-b                                 │
  │ Smoke test   will run (up to 30s)                                      │
  │ Model probe  2 model(s) discovered                                     │
  └────────────────────────────────────────────────────────────────────────┘
```

Success panel after the smoke run:

```text
  ┌─ Ready ────────────────────────────────────────────────────────────────┐
  │ ✓ Provider     openai-compatible (fake-model-a)                         │
  │ ✓ Runtime home runtime home exists                                      │
  │ ✓ Credential   provider does not require a key                          │
  │ ✓ Model list   2 model(s) advertised (verified by setup wizard)         │
  │ ✓ Model        fake-model-a                                             │
  │ ✓ Permissions  safe (write: true, network: false)                       │
  │ ✓ Workspace    /.../workspace-b                                         │
  │ ✓ Smoke test   Reply with OK                                            │
  │ ✓ Doctor       overall: Ok                                              │
  └────────────────────────────────────────────────────────────────────────┘
```

Narrow 40×20 welcome (compact wordmark; shorter copy):

```text
  ────────────────────────────────────
              1 · A G E N T
     EVIDENCE-ACCOUNTABLE AGENT RUNTIME
            v0.98.0-dev · REAL WORLD
          Welcome. Let's get started.
           •  Pick a provider and model
        •  Choose permissions & workspace
             •  Run a short smoke test
         Enter begin · Ctrl-C quit
  ────────────────────────────────────
```

## File outcomes

- Fresh flow: `config.json` written, setup report `Ready`, smoke `Reply with
  OK`, exit 0.
- Existing-model flow: `config.json` model changed from `legacy-model` to
  `fake-model-a`; `status --json` returned `ok: true`.
- Idempotent rerun:

  ```text
  config_before=2df92974dec5b0ab4f738a22cc13e1b7d36af33d3b5094c7f0d00cef6951b355
  config_after =2df92974dec5b0ab4f738a22cc13e1b7d36af33d3b5094c7f0d00cef6951b355
  CONFIG_UNCHANGED
  ```

- Masked-key flow: `keys/provider-openai-compatible.key` is `-rw-------`
  (0600), 15 bytes, content matches the non-secret fixture
  `fixture-key-123`; `grep -c 'fixture-key-123'` on the full PTY capture
  returns `0`; `status --json` returns `ok: true` with
  `credential: present`.
- Probe-failure flow: the wizard offers Retry / Enter model id manually;
  choosing manual writes `manual-model`, `run_setup` skips the repeated probe,
  and `status --json` returns `ok: true` with `credential: not_required`.
- Cancel: exit `130`, `Setup cancelled.`, alternate-screen leave sequence
  present, runtime home left empty.
- Color: raw capture contains the TUI military-green RGB sequences
  `38;2;63;185;80`, `38;2;74;120;85` and `38;2;94;158;94`.
- Panel alignment: every `Ready` panel line (top border, check rows, bottom
  border) is exactly 80 columns in the 100-column PTY capture, so the right
  border does not drift.

## Automated gates

```text
./scripts/verify.sh                                ✅ verification passed
cargo fmt --check                                  ✅
cargo clippy --offline --all-targets -- -D warnings ✅
cargo test --offline --lib setup_wizard             ✅ 10 passed
cargo test --offline --test setup_cli               ✅ 11 passed
core eval                                          ✅ 8/8
claims                                             ✅ 251 (227 verified / 16 partial / 8 no evidence)
```

`scripts/verify.sh` now points `ONEAGENT_HOME` at a temporary empty home, so
a developer's configured provider cannot make the CLI/TUI tests non-hermetic.
The TUI spawns in `tests/cli_e2e.rs` also set a per-test home explicitly.

## Residual

- Five-user timing/feedback is still owner-dependent.
- The wizard renders Unicode block/box glyphs; it assumes a UTF-8 terminal,
  consistent with the rest of the TUI.
