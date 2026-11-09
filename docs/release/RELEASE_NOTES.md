# Release Notes — 0.95.0 R1 Candidate

Date: 2026-11-08  
Status: **release candidate, not publicly releasable**

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
- **Packaging**: reproducible Linux archive for publication, internal Windows
  build (untested, not published), release manifest, installer and third-party
  license inventory.

## Upgrade notes

- A v1 runtime home must run `1agent upgrade` before default key paths, export,
  restore or key escrow use it.
- `1agent upgrade --rollback` moves keys back to the v1 layout and requires
  `keys/` to contain only the expected key, ledger and lock files.
- The release manifest for this candidate is verified in development policy.
  Production signing, approval envelopes and an external registry are separate.

## Verification

- 646 tests / 29 suites, 3 ignored (loopback/HTTP/soak), plus explicit ignored
  HTTP and loopback runs.
- `scripts/ci.sh` status 0; `scripts/release-check.sh --soak` passes five soak
  cycles.
- One-hour soak: 2922 cycles / 3600 seconds on the frozen core binary.
- Linux package built for publication; Windows package is an internal untested
  build and is not published. `release verify` reports `verified` for the local
  dual-platform manifest (development policy). The current Linux archive still
  contains the Windows binary because both platforms share one package root; a
  platform-specific Linux archive and a Linux-only manifest are needed before an
  external release that omits the Windows asset.
- Install from the Linux archive was smoke-tested in a temporary prefix.

## Known limits

- A-Gate formal closure, external IdP, third-party security audit and the
  Workshop full release gate remain open.
- A real HTTPS alert receiver is not configured; delivery is best-effort.
- Cross-process indexes/automatic merge and multi-tenant authorization are not
  implemented.
- G2 daemon/Routine/Watchdog and a real cross-day task are future work.
- Unknown file formats and untrusted skills should not be run in the same home.

## License

MIT. See `LICENSE`, `THIRD_PARTY_NOTICES.md` and `THIRD_PARTY_LICENSES.md`.
