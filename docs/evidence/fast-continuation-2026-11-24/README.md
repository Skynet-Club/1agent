# Fast continuations — 2026-11-24

Fast routing now covers a safe continuation: a new Act message in an existing
session whose newest Act work ledger entry still has an unchanged artifact.

## Eligibility

`auto_fast_decision` requires all of:

- interaction mode Act;
- task profile Fast;
- for a continuation, an active artifact from the work ledger; a new task
  still rejects injected facts;
- not verify-only, not new-version;
- write permission on;
- not a page deliverable.

A continuation without an active artifact stays on Core 7/full. Standard
follow-ups that do not match the Fast profile (for example a short “改标题”
without a fast marker) also stay on Core 7; this first version does not widen
the task-profile classifier. Pages, high-risk/authority and new-version work
remain excluded.

## Prompt context

Fast continuations add three system blocks before the session turns:

- `FAST FOLLOW-UP` — resolve the target from the conversation and active
  artifact paths; stay inside the workspace; use read/write directly.
- the work ledger context (artifact paths, hashes, status);
- `CONVERSATION CONTEXT` with the session facts, capped at 4,000 characters.

New fast tasks keep the previous tiny prompt; the fixed-token benchmark shape
is unchanged.

## Tests

- `runtime::tests::auto_fast_decision_is_narrow` covers continuation with and
  without an artifact, continuation with facts, page continuation and the
  unchanged new-task rules.
- `runtime::tests::auto_fast_allows_a_continuation_with_an_active_artifact`
  runs a normal first task, verifies the work ledger, then runs a fast
  follow-up and checks `FastModeSelected`, the four-tool surface and the
  `FAST FOLLOW-UP` prompt.

## Live TUI check

Fresh tmux TUI session, auto-fast default; binary `2af507a9…`:

1. First task wrote `workspace/out.txt` (hello): **2.2 s, 2 steps, 1 tool**,
   delivery queued and receipt `accepted`.
2. Follow-up asked for hello on line one and world on line two: **2.2 s,
   2 steps, 1 tool**, journal
   `FastModeSelected {enabled: true, reason: "auto: fast continuation with
   active artifact"}`, exactly the four fast tools, a `FAST FOLLOW-UP` system
   message in the model request, delivery queued and receipt `accepted`.
3. Both receipts have 0 failed checks. Transcript: `tui-followup.txt`;
   receipts: `receipt-first.json`, `receipt-followup.json`.

## Binary

Final installed `~/.local/bin/1agent` sha256 `2af507a9e99afadbdaa57e73…`;
previous `eb9e6d54…` saved as
`~/.local/bin/1agent-pre-fast-continuation-2026-11-24`.
