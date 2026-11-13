# ADR 0128: TUI Provider Panel and Local Credentials

- Status: accepted
- Date: 2026-11-13
- Related: ADR-0127, provider configuration

## Context

Provider keys were configured only through environment variables or the
wrapper's private `~/.pi/agent/auth.json`. A wrong or expired key surfaced as a
provider 401 after the user had already started a task, and
`providers --check` reported "ready" because it only checked configuration
presence. The model, API key and thinking depth also had no single in-product
surface.

## Decision

1. `/model` exposes provider setup actions in the existing command panel.
   Selecting "设置 API Key 与模型…" enters a masked API-key input in the
   composer; the key is never written to the transcript or run journal.
2. Saving the key first validates it against the provider's OpenAI-compatible
   `/v1/models` endpoint. The returned model IDs populate a selectable model
   menu instead of a hard-coded flash/pro list.
3. The selected model and an optional `reasoning_effort` (`low`, `medium`,
   `high`, or off) are applied by rebuilding the provider and probing it. If
   the provider rejects the field, the setting is not saved.
4. The key is stored at `~/.1a/keys/provider-<id>.key` with mode `0600`. The
   non-secret profile is stored at `~/.1a/provider-profile.json` with mode
   `0600`.
5. Startup applies the saved profile and key over environment variables and the
   wrapper's `~/.pi/agent/auth.json`. The OpenAI-compatible wire request carries
   `reasoning_effort` only when selected.

## Consequences

- Users can repair a 401 from the TUI without editing environment variables or
  wrapper files.
- Secrets stay out of config JSON, transcripts and journals. The model panel
  shows presence, never the value.
- Provider support for `/v1/models` and `reasoning_effort` is required for the
  full flow; unsupported endpoints or fields fail visibly instead of being
  silently accepted.
- The store is a local file, not an OS keyring. The profile is single-provider
  in this slice; multi-provider profiles and key rotation are follow-ups.
