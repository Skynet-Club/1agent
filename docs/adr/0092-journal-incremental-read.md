# ADR 0092: Journal Incremental Read

- Status: accepted
- Date: 2026-11-12

## Context

`read_events` re-read and re-parsed the whole JSONL journal on every call. A
long-running TUI or daemon that refreshes a projection therefore pays for the
entire history each time, and a 1M-token run makes that cost visible. The
journal already has a contiguous `seq`, one event per line, and `append` writes
and flushes a full line.

## Decision

1. Add `JournalCursor` (byte offset plus the next global line number) and
   `read_events_incremental(path, &mut cursor)`.
2. Incremental reads parse only newline-terminated lines after the cursor.
   A trailing line without a newline is not consumed, so a reader racing a
   writer cannot parse half an event. The cursor advances by the bytes and
   lines actually consumed.
3. A cursor below the last read offset (the file shrank or was replaced)
   returns `JournalReadError::Truncated` instead of silently re-reading.
4. `read_events` keeps its full-file contract but parses from a `BufReader`
   with `read_until`, avoiding one `String` allocation per line. It still
   accepts a final line without a newline.
5. JSON errors report the global line number, including lines skipped because
   they were blank.

## Consequences

- A caller that keeps a cursor sees O(new events) work instead of O(history).
- No existing consumer is switched to the cursor yet. TUI projections and
  daemons still call `read_events`; the cursor is the primitive they can adopt
  without changing their parsed-event contract.
- The journal format does not change, so existing files and replay are
  unaffected.
- A truncated or replaced journal must be re-read with a fresh cursor; the
  cursor deliberately fails closed rather than guessing.
- Incremental reads still load the unread tail into memory. A reader that
  falls arbitrarily far behind pays for that tail, but never for history it
  has already consumed.

## Evidence

`cargo test incremental_read --lib` and two ignored timing probes:
a 4.7 MB journal (~1.2M ASCII tokens) parses in 31 ms in a debug build, and
appending one event to a 23.7 MB, 20k-event journal is read in about 20 µs.
