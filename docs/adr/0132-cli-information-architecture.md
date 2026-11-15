# ADR-0132: CLI information architecture and public/advanced command split

- Status: proposed
- Date: 2026-11-14
- Supersedes: none
- Related: ADR-0045 (onboarding v0), ADR-0097 (actionable error recovery),
  `docs/0.98-onboarding-and-cli.md`, `docs/0.98-development-plan.md`

## Context

The 0.98 tree has 81 top-level commands and 117 nested subcommands in one flat
clap enum (`src/main.rs:59-70`). `1agent --help` exposes every internal and
operator command, including one-off audit, release, master and evaluation
commands. Only `--allow-write` is hidden, and it is ignored.

This is a discoverability and safety problem:

- a new user cannot tell which 10–15 commands are the product;
- internal commands look equally supported and equally safe;
- help output is too large to scan;
- shell completions and a man page would expose the same noise.

The handoff requires a public layer (`setup`, `tui`, `run`, `doctor`, `model`,
`permissions`, `audit`, `replay`, `skills`, `schedule`, `serve`, `uninstall`)
and advanced commands behind `help advanced`/`--all`.

## Decisions

1. **Public command set is fixed at 13**: `setup`, `status`, `tui`, `run`,
   `doctor`, `model`, `permissions`, `audit`, `replay`, `skills`, `schedule`,
   `serve`, `uninstall`. `help` and `version` remain built-in.
2. **Advanced commands are hidden, not removed.** Every non-public top-level
   command gets `hide = true`. They remain executable, scriptable and covered
   by tests.
3. **A custom `help` subcommand replaces clap's built-in help subcommand.**
   - `1agent help` → public help.
   - `1agent help advanced` → grouped advanced commands with one-line summaries.
   - `1agent help --all` → every command, grouped.
   - `1agent help <command>` → that command's help.
   - `-h/--help` keeps clap's standard behavior.
4. **Advanced groups are stable names**, used by both help and docs:
   `provider`, `memory`, `trust`, `ops`, `master`, `evolution`, `protocol`,
   `eval`.
5. **Renames are aliases, not deletions.** `skills` is the primary command;
   `skill` remains an alias with a one-line stderr notice. `init` stays
   advanced and is called by `setup`; `onboard` stays advanced with its
   UserModel/Goal semantics.
6. **New commands live in separate modules** (`src/cli_setup.rs`,
   `cli_status.rs`, `cli_permissions.rs`, `cli_help.rs`, `cli_output.rs`,
   `provider_profile.rs`). `main.rs` only adds clap variants and dispatch arms,
   to keep parallel work mergeable.
7. **No behavior change for existing commands in this ADR.** `audit`/`replay`
   defaulting to the last run is a separate change with its own tests.

## Consequences

- `1agent --help` shrinks to the public product surface; advanced users need
  one extra command (`help advanced`) but get grouping and summaries.
- Existing scripts keep working because no command is removed.
- Hidden commands still need tests and documentation; hiding is a UX rule, not
  an authorization boundary. Authority/Policy checks remain the enforcement.
- Shell completions and the man page will include hidden commands only if the
  generator includes them; the default should include public commands, with
  `help --all` for the rest.
- The custom help subcommand must be tested against clap's `-h`, `--help`,
  missing-subcommand and unknown-subcommand paths.

## Tests

- `1agent --help` lists exactly the public commands and not `master`/`watchdog`.
- `1agent help advanced` lists all eight groups and representative commands.
- `1agent help --all` lists at least one command from every group.
- `1agent skill --help` still works and prints the alias notice on stderr.
- `1agent help <unknown>` exits 2 with `error:` + `next:`.
- Snapshot test for help output width and `NO_COLOR`.
