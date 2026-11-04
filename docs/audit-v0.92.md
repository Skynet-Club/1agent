# Audit v0.92.0

Date: 2026-11-02

## Scope

- frozen Skynet Terminal: not modified
- 1A runtime/provider contracts: added invalid-final preservation and parse repair; tool contracts
  are unchanged
- presentation layer: replaced with Ratatui and Crossterm
- OpenAI Codex source: interaction concepts and selected algorithms only

## Verification snapshot

| Check | Result |
|---|---|
| Formatting | passed |
| Clippy with `-D warnings` | passed |
| Unit tests | 163 passed |
| TUI rendering tests | 4 passed |
| Offline integration / CLI E2E | passed |
| Rust API documentation | passed |
| Release-profile build | passed |
| Input mouse selection and clipboard copy | passed in PTY |
| Transcript mouse selection | passed in PTY |
| Transcript selection edge autoscroll | passed |
| User/assistant vertical separation | passed |
| Compact prose and code-section layout | passed |
| Codex-style added/removed line backgrounds | passed |
| Slash command drawer | passed in PTY |
| Bracketed paste | passed in PTY |
| Journal audit smoke | passed |
| Pi DeepSeek probe | passed |
| Real model basic conversation | passed |
| Expanded language-label recognition | passed |
| Code-fence border and language-label color separation | passed |
| Function, statement, class/type, string, number, comment and reference highlighting | passed |
| Quote and query-result rendering | passed |

## Requirement alignment

- Input editing uses Unicode grapheme boundaries.
- Mouse capture prevents terminal-native selection from copying composer padding.
- Releasing a non-empty mouse selection copies only the selected text.
- Right-click dispatches clipboard paste through the host clipboard adapter.
- Input and transcript selection copy only explicit text ranges.
- The latest user instruction remains visible.
- The sticky prompt bar follows the visible answer and yields when its source prompt is visible.
- The interactive surface no longer reserves a global status row; operational information remains
  available through `/status`.
- Slash commands are filterable and keyboard-selectable.
- Transcript layout is structured, width-aware and scrollable.
- Transcript headings, labels, evidence, descriptions and metadata use semantic tones derived
  from Codex's accent/status styling model.
- Scrolled transcripts expose `New activity` and `Back to bottom` affordances without changing
  journal or selection state; pointer hover uses reverse-video feedback.
- TUI sessions persist presentation metadata and journal associations; journals remain the
  authoritative record.
- Continued requests include at most six recent turns, with per-fact and total context limits.
- The Pi-backed launcher supplies a real DeepSeek credential without copying the secret into
  repository config or journal.
- The welcome surface is presentation-only and disappears once transcript content begins.
- The welcome wordmark highlights only the leading `1` contour in green; the two left-edge
  horizontal bars remain blue.
- The post-exit Session/Continue banner reuses the same colored wordmark and sanitizes session
  metadata before writing to the restored terminal.
- Direct answers hide repetitive schema and operational metadata from the primary transcript while
  preserving it through `/details` and the journal.
- Completed direct answers do not render evidence or caveat sections; user-facing prose is not
  replaced by internal schema terminology.
- The provider prompt explicitly enforces empty sources for `model_prior` claims and empty
  `action_receipts` for read-only tasks.
- Evidence timestamps must be omitted or `null` when unknown; present expiration timestamps must
  follow the observation timestamp.
- Non-JSON final text is retained in the journal and receives the same bounded correction request
  as schema-validation failures. The corrected response must parse and pass the full domain
  validator before it can become an answer.
- Standard Markdown fences, standalone language labels followed by code, and obvious unlabelled
  code use the same full-row dark surface with visible `╭─ / │ / ╰────` boundaries.
- Ordinary prose remains outside inferred code blocks.
- Code labels cover 163 common language, markup, data-format and tooling aliases.
- Markdown blockquotes are grouped into a dedicated quote surface rather than ordinary answer text.
- Query results are reconstructed from journaled tool observations, carry source/status labels and
  a non-original disclosure, and summarize search hits without exposing raw JSON in the transcript.
- Query receipts and zero-match scan status are omitted from the primary conversation and exposed
  only through the non-mutating `/details` overlay and journal.
- Quote and query-result content uses gray dim styling as supplementary evidence rather than
  competing with the primary answer.
- Running activity separates a bright-green spinner from a bright-blue status label.
- Composer insertion preserves literal spaces; user messages and code blocks use full-row
  backgrounds without copyable padding text.
- Runtime, provider, permission and journal behavior remain outside presentation state.

## Remaining evidence gaps

- live provider token streaming;
- Markdown/code/tool result cards;
- transcript search and keyboard copy navigation;
- macOS and Windows interactive terminal verification;
- real-model conversation quality without configured credentials.
