# Release Notes — 0.95.0 R1 Candidate

Date: 2026-11-09  
Status: **R1 public pre-release (release candidate), not production-stable**

GitHub pre-release: <https://github.com/Skynet-Club/1agent/releases/tag/v0.95.0-rc.1>

[简体中文](https://github.com/Skynet-Club/1agent/blob/main/docs/release/RELEASE_NOTES.md) · **English**

## Development status and community

This project is still under active development. 0.95.0 is an R1 release
candidate, not a finished or production-stable product; interfaces, configuration
and data formats may still change.

We welcome developers, testers, researchers, security specialists, product and
design partners to test the Linux build, report issues, submit pull requests, and
join **Skynet-Club** to help build the product and ecosystem:
<https://skynet.club>.

The future direction is open and iterative. Browser, subagent, always-on
autonomy, tool ecosystem, Workshop and Bot capabilities have a public capability
plan; each stage advances through testable milestones and evidence. Plans can be
discussed and revised; the roadmap is not a one-time promise.

## Highlights

- **Runtime home**: `init/doctor/export/restore/upgrade/uninstall` around
  `$ONEAGENT_HOME` or `$HOME/.1a`.
- **Home layout v2**: master key and KEK ledger live under `keys/`; v1 homes
  migrate with `1agent upgrade` and can be reversed with
  `1agent upgrade --rollback`.
- **Key escrow**: AES-256-GCM escrow of the master key and ledger under a
  separate 0600 recovery key.
- **Cross-process safety**: flock and stale-writer detection for task-graph,
  memory, skill, goal, action and snapshot stores; long-lived surfaces reload
  and return retryable conflicts instead of wedging.
- **Alerts**: local JSONL alert log plus optional HTTPS webhook for provider
  health events.
- **Restart continuity**: goals, commitments, memory, delegation and evolution
  survive a full reopen; the context graph explains goal/commitment links.
- **TUI**: standalone turns no longer open a modal `/next` menu; `/` commands
  remain discoverable; greetings/connectivity checks stay tool-free. The mode
  menu is reachable by `/mode`, Ctrl+T (portable) or Ctrl+Tab, with six modes.
  The slash drawer supports fuzzy/name+description matching, disabled-command
  reasons, Tab argument completion and non-executing completion on Enter. The
  empty drawer is grouped by category with the three most recent commands first;
  command panels own the keyboard; destructive clear/dispatch/control actions
  require a second Enter.
- **TUI session and status**: a bare `1agent tui` starts a new session; the
  post-exit banner reports the session title and explicit `--session` continue
  command. `Ctrl+Q` requires Enter confirmation, and the running line shows
  `Working...`, elapsed time, input/output tokens and provider cache hit rate.
- **Direct answers and run metrics**: Direct Answer mode shows only the conclusion
  in the transcript; structured evidence stays in `/details`. Providers that do
  not report cache statistics render `cache —` instead of a fabricated rate.
- **Provider cache usage**: OpenAI-compatible and DeepSeek cache-token fields are
  parsed into `Usage`.
- **Packaging**: reproducible Linux-only archive for publication, internal Windows
  build (untested, not published), `SHA256SUMS`, provenance, installer and
  third-party license inventory.

## Published assets

- `1agent-0.95.0-linux-x86_64.tar.gz` — Linux x86_64 package;
- `SHA256SUMS` — archive and Linux provenance checksums;
- `linux-x86_64.json` — Linux build provenance.

Windows archives and the dual-platform manifest are intentionally not attached.
GitHub's generated source archives are also available from the tag.

## Upgrade notes

- A v1 runtime home must run `1agent upgrade` before default key paths, export,
  restore or key escrow use it.
- `1agent upgrade --rollback` moves keys back to the v1 layout and requires
  `keys/` to contain only the expected key, ledger and lock files.
- The local dual-platform release manifest remains an internal verification
  artifact; it is not part of this Linux-only public pre-release. Production
  signing, approval envelopes and an external registry are separate.

## Verification

- 648 tests / 29 suites, 3 ignored (loopback/HTTP/soak), plus explicit ignored
  HTTP and loopback runs.
- `scripts/ci.sh` status 0; `scripts/release-check.sh --soak` passes five soak
  cycles.
- One-hour soak: 2922 cycles / 3600 seconds on the frozen core binary.
- A platform-specific Linux-only archive is built for this pre-release; it does
  not contain `windows-x86_64/1agent.exe`.
- Windows remains an internal, untested build and is not published.
- The local dual-platform `release verify` status remains an internal check; the
  public pre-release uses the Linux archive, `SHA256SUMS` and Linux provenance.
- Install from the Linux archive was smoke-tested in a temporary prefix.

## Known limits

- A-Gate formal closure, external IdP, third-party security audit and the
  Workshop full release gate remain open.
- A real HTTPS alert receiver is not configured; delivery is best-effort.
- Cross-process indexes/automatic merge and multi-tenant authorization are not
  implemented.
- Browser/computer-use, generic subagents and MCP are not implemented.
- G2 daemon/Routine/Watchdog and a real cross-day task are future work.
- Windows is untested and not published; macOS, Android and iOS are planned.
- Unknown file formats and untrusted skills should not be run in the same home.

## License

MIT. See `LICENSE`, `THIRD_PARTY_NOTICES.md` and `THIRD_PARTY_LICENSES.md`.
