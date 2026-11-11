# ADR 0036: Platform Capability Registry

- Status: accepted
- Date: 2026-11-11

## Context

The product is preparing for Windows and macOS, but platform support was
described in scattered prose. Some capabilities are verified only on Linux,
some are degraded, some are unsupported, and most are untested elsewhere. A
release must not turn "it builds" into "it is supported".

Domain code must stay platform-neutral. Platform differences belong in
adapters, with an explicit status that users and the release gate can inspect.

## Decision

1. Add `src/platform.rs` with:
   - `Platform` (`linux`, `macos`, `windows`, `other`);
   - `CapabilityStatus` (`verified`, `degraded`, `unsupported`, `untested`);
   - `Capability` (name, status, note);
   - `PlatformReport` (platform, os, arch, capabilities).
2. `report()` describes the current host; `report_for(platform)` is testable
   without running on that host.
3. Linux reports verified core paths (`runtime_home`, `secure_file`,
   `clipboard`), degraded adapters (PTY via Workshop, browser read-only,
   daemon manual, webhook-only notifications, HMAC signing, Linux-only
   package) and unsupported computer-use.
4. macOS, Windows and other targets report every capability as `untested`.
   They must never claim `verified` until the P-Gate exercises them.
5. Add `1agent platform [--json]` as the inspectable surface.
6. Domain code must not branch on `cfg(target_os)`; platform branches stay in
   adapters. This ADR adds a registry, not a second execution path.

## Consequences

- Release notes can link to a machine-readable platform report instead of
  claiming support from a successful compile.
- The Windows/macOS P-Gate has a fixed list to promote: runtime home, secure
  files, PTY/ConPTY, browser, service, notifications, clipboard, signing and
  packaging.
- `untested` is a block, not a soft warning: no support claim, no download.

## Evidence

```bash
cargo test platform --lib
cargo test --test platform_cli
```

The tests cover the Linux status map, the "untested never claims verified"
rule, JSON round-trip and both CLI output modes.
