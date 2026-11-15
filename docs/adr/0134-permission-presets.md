# ADR-0134: Permission presets

- Status: proposed
- Date: 2026-11-14
- Supersedes: none
- Related: ADR-0031 (execution by default), ADR-0044 (autonomy dial),
  ADR-0118 (kill switch), ADR-0133 (setup/config), ADR-0135 (browser action
  cards), `docs/0.98-onboarding-and-cli.md`

## Context

0.97 has the ingredients but no product-level presets:

- `PermissionSet { read_roots, allow_network, allow_write }`
  (`src/domain.rs:86-97`).
- `--read-only`, `--allow-network`, `--read-root` (`src/main.rs:4102-4116`).
- Optional `Authority` profiles with scopes, `max_strength`, allowed/denied
  tools and `max_risk` (`src/authority.rs:38-130`).
- `RequireApproval` becomes `approval_required` and **fails closed** in the
  generic execution path (`src/runtime.rs:304-339`).
- No `read-only`/`safe`/`full-local` presets; the TUI has per-run
  `/permissions` toggles; headless `run` has no interactive approval loop.

The 0.98 handoff asks for three presets: read-only / safe (write + approval) /
full local. The risk is inventing a second policy path that drifts from
`PermissionSet`/`Authority`.

## Decisions

1. **Three presets, implemented as sugar over the existing gate.**
   `read-only` / `safe` / `full-local` compute a `PermissionSet` and an
   `Authority` profile. No new runtime check is added.
2. **`read-only`**: `allow_write=false`, `allow_network=false`,
   `read_roots=[workspace]`. Write and terminal tools are hidden; a write
   attempt returns `error: permission.read_only` with
   `next: use a writable preset`.
3. **`safe`**: `allow_write=true` within the workspace; `allow_network=false`
   by default. Network/execute/high-risk actions require approval. In headless
   mode, `RequireApproval` returns `approval_required` and does not execute;
   it never auto-approves.
4. **`full-local`**: `allow_write=true` for the workspace and explicit roots;
   `allow_network=true`; routine actions are not prompted. **Irreversible
   actions still require confirmation** (browser slice), and `KILL` still
   blocks write tools. Full-local is not "no policy".
5. **Presets are stored in config v2** (`permissions.preset`,
   `permissions.read_roots`, `permissions.allow_network`). The resolved
   `PermissionSet` is derived at load; `permissions explain` prints the
   derivation and any drift.
6. **`permissions set` never silently widens access.** Enabling network
   requires `--network on` (or `--allow-network` on `run`); enabling
   `full-local` prints a warning that irreversible actions still confirm.
7. **KILL is orthogonal.** Every preset checks `$ONEAGENT_HOME/KILL` before
   write tools; read-only remains available. The kill switch is not a preset
   and cannot be disabled by a preset.
8. **No model-judged permissions.** The preset and the local policy decide;
   model text cannot raise a preset, grant network, or approve an action.

## Consequences

- Users get a simple three-way choice; operators can still supply a full
  `Authority` profile, which is intersected with the preset.
- `safe` is intentionally stricter than "write + approval": network is off
  until explicitly enabled, because most tool exfiltration paths are network
  paths.
- Headless `safe` runs will fail closed on actions that need approval. This is
  correct but may surprise users; the error must name the exact
  `--allow-network`/approval path.
- `full-local` still prompts for irreversible browser actions; this is the
  product boundary agreed for 0.98.
- Tests that assume "write on by default" keep working, but tests that assert
  network on by default must be updated only if the config preset says so.

## Tests

- Table test: each preset maps to the expected `PermissionSet` and
  `Authority` scopes.
- `read-only` write attempt fails with `permission.read_only`; no file changes.
- `safe` headless network action returns `approval_required`; no network call.
- `safe` + explicit `--allow-network` performs the action; journal records the
  grant.
- `full-local` write/network succeeds; an irreversible browser action still
  returns `approval_required`.
- KILL blocks write in all three presets; read-only read still works.
- `permissions explain --json` contains no secret and matches the resolved
  config.
