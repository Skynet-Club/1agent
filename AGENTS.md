# 1·Agent Repository Rules

Read `/home/skynet/AGENTS.md` first.

Project-specific rules:

- The frozen Skynet Terminal `v0.10.21` must not be modified for this project.
- Domain code must remain independent of UI, operating system, database and model-provider SDKs.
- Read-only behavior is the V0 default. New side effects require an explicit permission model and tests.
- Every model-visible input must be reconstructable from the run journal.
- Tool calls and tool results must be paired in the journal.
- Never log credentials or full secret-bearing request headers.
- Prefer deterministic tests and fake providers over live network calls.
- Update `docs/architecture.md`, ADRs and the Obsidian `1·Agent` note when contracts change.
- Use the development flywheel: requirement -> design -> implementation -> test -> verification -> audit -> document -> next requirement.
