# ADR-0133: Setup state machine, config v2 and secret input

- Status: proposed
- Date: 2026-11-14
- Supersedes: none (extends ADR-0045 onboarding v0, ADR-0024 runtime home)
- Related: ADR-0132 (CLI IA), ADR-0134 (permission presets),
  `docs/0.98-onboarding-and-cli.md`

## Context

0.97 has no `setup` command. `init` creates a private runtime home and is
idempotent (`src/runtime_home.rs:188-228`), but provider configuration is only
loaded from an explicit `--config` (`src/config.rs:85-90`). The nearest command,
`onboard`, checks paths and env only, does not create a home, does not save a
key, and appends a UUID record on every run (`src/main.rs:2825-2847`), so it is
not idempotent.

Provider keys are accepted via `--api-key` (argv), `AGENT_API_KEY`, or a 0600
file only in the TUI flow (`src/main.rs:12649-12733`). The TUI already knows how
to probe `/models`, select a model and save a key at
`~/.1a/keys/provider-<id>.key` 0600.

The 0.98 acceptance requires: new user completes a first task in ≤5 minutes
with no manual file edits; setup is idempotent; read-only refusal is explicit.

## Decisions

1. **`setup` is an orchestration layer over existing primitives**, not a new
   runtime: `runtime_home::init` → provider → credentials → model →
   permissions → workspace → smoke → doctor.
2. **`setup.json` records the state machine** at
   `$ONEAGENT_HOME/setup.json` (0600, atomic temp+fsync+rename). It contains
   `schema`, `current_step`, `completed[]`, `config_hash`, `updated_at`,
   `last_error`. It never contains a secret.
3. **`--check` is read-only.** It computes desired state, compares with
   `config.json`/`setup.json`/key files, and exits 0 (ready) or 1 (not ready)
   without writing or prompting. It does not probe the network unless
   `--check --probe` is passed.
4. **`--non-interactive` never prompts.** Missing required values exit 2 with
   `error:` + `next:`. With no TTY and no flags, `setup` exits 2 instead of
   blocking. `--yes` accepts defaults only; it does not invent a provider,
   model or key.
5. **Config v2 adds optional fields** while remaining backward compatible:
   `schema_version`, `default_provider`, `workspace`, `permissions`, and
   `ProviderConfig.api_key_file`. `deny_unknown_fields` stays. v1 configs load
   with defaults and are not rewritten until a setup/permissions command
   updates them.
6. **Default config path** is `$ONEAGENT_HOME/config.json` if it exists;
   `--config` still overrides. If neither exists, existing flag/env behavior is
   unchanged.
7. **Key input is stdin or masked prompt, never argv.** `setup` rejects
   `--api-key <value>` with `setup.api_key_argv_forbidden`; the supported
   automation path is `--api-key-stdin`. Keys are written 0600 with
   `O_NOFOLLOW`, refuse symlinks and atomic replace; config stores
   `api_key_file`, not the key.
8. **Key precedence**: explicit CLI flag (legacy, deprecated) > `api_key_file`
   > `api_key_env` > missing.
9. **Provider probe**: 5s timeout, redirects disabled, HTTPS except loopback,
   bearer only when a key exists, `/models` (or `/v1/models`) with an Ollama
   `/api/tags` fallback. Empty model list requires an explicit `--model`.
10. **30-second smoke task** is a single provider call with a 16-token cap. It
    writes a journal. `--skip-smoke` records `smoke: skipped`; `setup --check`
    reports `smoke: unverified`.
11. **`setup` writes a default `config.json`**, but `--config` remains the
    override. This is the first default config path in the product.
12. **No OS keyring in 0.98.** The canonical store is the 0600 file. A keyring
    backend is a 0.98.1 candidate; adding it now would create a headless/D-Bus
    dependency and a second source of truth.

## Consequences

- Existing users with `--config` or env-only providers keep working; they are
  not forced through `setup`.
- A second `setup` must produce 0 byte changes to config/key files (except
  `setup.json.updated_at`); tests compare hashes.
- `setup --check` cannot detect a provider that is down unless `--probe` is
  explicit; this keeps checks deterministic and offline-safe.
- The default config path changes startup behavior when
  `$ONEAGENT_HOME/config.json` exists. This must be documented in the manual
  and release notes.
- `--api-key` remains on legacy commands for compatibility but is deprecated
  and must not be used by setup or new docs.
- No keyring means a compromised user account can read the key file; the 0600
  mode and `O_NOFOLLOW` are the boundary, not encryption at rest.

## Tests

- `setup --check` on a fresh home: exit 1, JSON `ok=false`, no files created.
- `setup --non-interactive` without provider: exit 2, `error:` + `next:`, no
  hang under a closed stdin.
- Two consecutive non-interactive setups with a fake provider: second run
  changes 0 bytes in `config.json` and key file; exit 0.
- `setup --api-key secret`: exit 2, stderr does not contain `secret`.
- `--api-key-stdin`: key lands at `keys/provider-<id>.key` 0600; JSON and
  journal contain no key.
- Probe fixtures: OpenAI shape, Ollama tags, 401, 302, empty list, timeout.
- `setup --resume` after a simulated interrupt continues from `setup.json`.
- v1 config loads and `setup --check` reports only the missing v2 fields.
