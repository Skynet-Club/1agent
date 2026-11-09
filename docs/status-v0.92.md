# Status v0.92.0

Date: 2026-11-02
Status: Ratatui/Crossterm interaction layer implemented

## Interface

`1agent tui` now uses a Ratatui terminal buffer and Crossterm event stream. The interactive Unix
surface includes:

- a viewport-aware sticky prompt bar that yields when the source prompt is visible;
- grapheme-aware editing with horizontal cursor visibility;
- a centered three-row dark-gray composer with a bold bright-green Codex-style `›` prefix;
- a separate two-row black shortcut footer;
- a filtered `/` command drawer with keyboard selection and Tab completion;
- structured transcript wrapping, scrolling and text selection;
- Codex-style semantic transcript hierarchy and color tones;
- dedicated quote styling and low-emphasis non-original query receipts with journal provenance;
- 163 code-language and format labels;
- Codex-style `New activity` / `Back to bottom` follow control;
- persistent TUI sessions with journal-backed restoration and continue instructions;
- bounded prior-turn context injection for continued conversations;
- responsive ASCII welcome surface with session and resume guidance;
- conversation-first adaptive answer formatting with non-transcript `/details` and `/journal`;
- mouse capture, internal selection, copy-on-select and explicit clipboard copy/paste;
- bracketed paste and graceful cancellation.

The line-oriented fallback remains available for pipes, CI and hosts without a verified interactive
terminal.

## Verification

| Check | Result |
|---|---|
| Unit tests (`src/lib.rs`) | 163 passed |
| TUI rendering tests (`src/main.rs`) | 4 passed |
| Integration / E2E | passed offline |
| Clippy `-D warnings` | passed |
| Rustdoc `-D warnings` | passed |
| Release build | passed |
| Input mouse selection and copy | passed in PTY |
| Transcript mouse selection and copy | passed in PTY |
| Mouse-selection edge autoscroll in both directions | passed |
| Copy-on-select mouse release | passed in PTY |
| Right-click paste dispatch | passed in PTY |
| `/` command drawer and Tab completion | passed in PTY |
| Sticky prompt bar follows and yields with transcript position | passed in PTY |
| Global status row removed; `/status` drawer command | passed |
| Mouse-selection copy confirmation | passed in PTY |
| Follow control and Esc return-to-bottom behavior | passed in PTY |
| Follow-control hover highlighting | passed |
| Session exit summary and `--session` transcript restoration | passed in PTY |
| Colored post-exit welcome wordmark | passed |
| Adaptive direct/structured answer rendering | passed |
| Real model self-introduction with strict claim sources | passed |
| Invalid JSON parse and schema-repair/revalidation | passed |
| Raw invalid provider text preserved in journal | passed |
| Standard, bare-label and inferred code block formatting | passed |
| Bright-green code-fence and blue language-label colors | passed |
| IDE-style code tokens and reference highlighting | passed |
| Composer space input | passed |
| User-message surface padding and status spacing | passed in PTY |
| Black gap between user message and assistant answer | passed |
| Compact paragraph/code-block spacing | passed |
| Green/red diff line backgrounds | passed |
| Prior-turn context in resumed request journal | passed in PTY |
| Welcome surface rendering | passed in PTY |
| Welcome `1` glyph highlight | passed in PTY |
| Pi DeepSeek `deepseek-flash` provider probe | passed, 3257 ms |
| Real 1-Agent natural-language completion | passed |
| Default `1agent run` launcher | passed |
| Semantic transcript colors and hierarchy | passed in PTY |
| Expanded bare language labels (163 aliases) | passed |
| Green run spinner and blue running label | passed |
| Gray/dim quote and query-result supplementation | passed |
| Markdown quote surface | passed in TestBackend |
| Journal-backed human-readable query results | passed |
| Query receipts hidden from primary transcript | passed |
| Scrollable `/details` overlay without transcript mutation | passed |
| Bracketed paste | passed in PTY |
| Journal generation | passed |

## Known limitations

- model output is not token-streamed yet;
- only basic live-model connectivity and response behavior are verified, not broad reasoning quality;
- transcript rendering is structured plain text rather than Markdown cards;
- transcript search and keyboard copy mode are not implemented;
- Windows and macOS interactive behavior still require target-host verification.

## Next requirement

Add runtime event streaming and render long model/tool output incrementally without losing the
current selection, scrolling and journal invariants.
