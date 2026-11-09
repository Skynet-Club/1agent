# Handoff: v0.92.0

Date: 2026-11-02

## Current work cycle

- version: `0.92.0`
- repository status: uncommitted working tree
- release tag: not created
- frozen terminal: not modified

## Completed

- replaced the custom ANSI painter with Ratatui/Crossterm;
- added grapheme-aware editing and paste handling;
- added mouse selection for input and transcript;
- added copy-on-select, native clipboard paste and OSC 52 copy fallback;
- added the filtered `/` command drawer;
- added a viewport-aware sticky prompt bar that yields at visible prompt boundaries;
- removed the global status row in favor of `/status` and the welcome surface;
- added structured transcript wrapping and scrolling;
- added semantic transcript hierarchy and a documented Codex-aligned color palette;
- added visible code fences for standard Markdown, bare language labels and obvious unlabelled
  code while keeping surrounding prose in the normal transcript;
- expanded code labels past 100 aliases and added dedicated quote and non-original query-result
  surfaces backed by journaled tool observations;
- added the Codex-style follow control and persistent session/continue flow;
- added bounded prior-turn context injection for resumed conversations;
- added the responsive ASCII welcome/session surface;
- added adaptive conversation-first answer formatting and on-demand structured details;
- added parse-failure recovery that preserves malformed model text in the journal and requests one
  corrected AgentAnswer JSON response from the same provider;
- recorded OpenAI Codex and Ratatui attribution.

## Verification evidence

- 150 unit tests passed;
- all offline integration and CLI E2E tests passed;
- one loopback-only test remains conditionally ignored;
- Clippy and Rustdoc pass with warnings denied;
- release binary reports `1agent 0.92.0`;
- manual PTY checks cover input, transcript, clipboard copy, slash completion and bracketed paste.
- Pi DeepSeek probe and real-model basic conversation smoke passed.

See [`status-v0.92.md`](status-v0.92.md) and [`audit-v0.92.md`](audit-v0.92.md).

## Next action

Introduce runtime event streaming, then render incremental model text and tool activity through the
structured transcript without disrupting scroll or selection state.
