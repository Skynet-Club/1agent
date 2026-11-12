# Journal Incremental Read — Evidence (2026-11-12)

Status: **passed on this machine**
Scope: `JournalCursor`, `read_events_incremental`, and the buffered
`read_events` parser.

## Commands

```bash
cargo test incremental_read --lib
cargo test journal_full_parse_at_one_million_tokens --lib -- --ignored --nocapture
cargo test incremental_read_benchmark --lib -- --ignored --nocapture
```

## Results

```text
journal bytes=4734890 events=4000 full=31.07939ms
journal bytes=23689069 full=157.643837ms first=219.378636ms incremental=19.947µs
incremental_reads_only_new_events ... ok
incremental_read_holds_a_partial_trailing_line ... ok
incremental_read_reports_the_global_line_number ... ok
incremental_read_rejects_a_shrunk_journal ... ok
```

## What this proves

- A second incremental read returns only events appended after the cursor;
  a third read with no new data returns nothing.
- A line split across two writes is not parsed until its newline arrives; the
  cursor stays before it, then parses it once completed.
- A malformed line reports its global file line number, not an offset-local
  one.
- Truncating a journal below the cursor returns `Truncated`.
- `read_events` still returns all events, including a final line without a
  newline.
- A 4.7 MB journal (roughly 1.2M ASCII tokens) parses in 31 ms in a debug
  build, under the 50 ms SA97-10 acceptance size.
- After a 23.7 MB, 20k-event journal has been consumed once, reading one newly
  appended event takes about 20 µs.

## What this does not prove

- That any TUI or daemon projection uses the cursor today; they still call
  `read_events`.
- Release-build timings; the probes ran in a debug build.
- Bounded memory for a reader that falls arbitrarily far behind; the unread
  tail is read into memory.
- Concurrent writers to the same journal; `Journal` writes one line per run
  and does not implement cross-process locking.
