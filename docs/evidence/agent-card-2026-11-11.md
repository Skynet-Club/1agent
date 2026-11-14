# Agent Card v0 — Evidence (2026-11-11)

Status: **passed on this machine**  
Scope: `agent_card::build_card` and `1agent agent-card [--json]`. The card is
local and unsigned; it is not served over A2A/HTTP.

## Commands

```bash
cargo test agent_card --lib
cargo test --test agent_card_cli
cargo clippy --all-targets -- -D warnings
```

## Expected results

```text
running 3 tests
test agent_card::tests::card_has_required_fields_and_limits ... ok
test agent_card::tests::card_round_trips_through_json ... ok
test agent_card::tests::unsupported_claims_are_not_marked_verified ... ok

test result: ok. 3 passed; 0 failed

running 2 tests (tests/agent_card_cli.rs)
test agent_card_cli_reports_capabilities_and_limits ... ok
test agent_card_cli_human_output_lists_limits ... ok

test result: ok. 2 passed; 0 failed
```

## What this proves

- The card has schema version, name, version, platform, capabilities and
  non-empty limits.
- Core capabilities with contract tests are `verified`.
- `computer_use`/`pty` take their status from the platform registry; on Linux,
  computer-use is not `verified`.
- `cross_platform` is `untested`; `team_collaboration` and `marketplace` are
  `unsupported`.
- The card round-trips through JSON and both CLI output modes work.

## What this does not prove

- That the card is served or signed for A2A; it is a local file/stdout card.
- That the capability statuses remain true on other builds; they must be
  regenerated.
- Model quality or production readiness.
- Cross-platform behavior; the tests ran on Linux only.

## Files

- `src/agent_card.rs`
- `src/main.rs` (`agent-card`)
- `tests/agent_card_cli.rs`
- `docs/adr/0047-agent-card-v0.md`
- `docs/architecture.md`, `docs/features.md`, `docs/features.en.md`,
  `docs/claims.md`, `CHANGELOG.md`
