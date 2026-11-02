# Interactive TUI

Date: 2026-11-02

Status: V0 line-oriented terminal interface implemented

## Start

```bash
cargo run --offline -- tui
```

With a provider configuration:

```bash
cargo run --offline -- tui --config configs/example.json
```

The packaged Linux binary provides the same command.

## Layout

```text
1·Agent | status=running | mode=decision_analysis | risk=medium | provider=... | budget=6 steps/120s
 You > Compare A and B under a fixed budget
────────────────────────────────────────────────────────────
conversation and run output

>
```

The first line is the operational topbar. The second line always preserves the latest non-command
user instruction. It remains visible while the run executes and after a result or failure.

## Interaction

- type a task and press Enter to run it;
- `Ctrl+C` cancels the active run and preserves its journal;
- `/help` shows commands;
- `/clear` clears the transcript and last instruction;
- `/exit` or `/quit` ends the session.

Only one run executes at a time. The UI uses normal terminal line editing rather than raw-mode
key handling.

## Safety

- user instructions are sanitized before display;
- model/progress text is sanitized before terminal rendering;
- ANSI escape sequences, bidi controls and zero-width controls cannot be injected through the
  displayed instruction or transcript;
- provider credentials are not rendered;
- the existing read-only, network and journal policies remain unchanged.

## Current limitations

- line-oriented input, not full-screen raw-mode key handling;
- no mouse support;
- no scrollable panes or arrow-key navigation;
- no token streaming;
- task input waits for Enter rather than supporting multiline composition;
- Ctrl+C cancels a run but a second Ctrl+C is left to normal terminal process handling.

These are presentation limitations; the underlying runtime contracts remain the same.
