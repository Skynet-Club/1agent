# ADR 0094: TUI Transcript Projection Cache

- Status: accepted
- Date: 2026-11-12

## Context

`draw_transcript` rebuilt the wrapped visual transcript on every frame. A TUI
that redraws on a timer, cursor blink, activity animation or mouse move
therefore re-wrapped every entry even when nothing in the transcript had
changed. M15 requires a p95 frame time of 8 ms on a long transcript while a
run is active.

## Decision

1. The state keeps a `transcript_revision` counter, incremented whenever an
   entry is pushed or the transcript is cleared.
2. The cached projection stores the width and revision it was built from.
   `draw_transcript` rebuilds only when the width changed or the revision
   changed; otherwise it reuses the wrapped lines.
3. The existing 200-entry transcript cap and scroll/selection behavior are
   unchanged.
4. The projection cache covers the transcript only. Other panels keep their
   current rendering path.

## Consequences

- Idle frames and animation-only frames do not re-wrap the transcript.
- A new entry or a terminal resize rebuilds the projection, so the displayed
  text cannot go stale.
- M15 was measured with a 200-entry transcript and an update every fourth
  frame: release p95 1.26 ms, debug p95 11.1 ms. The 8 ms hard gate applies to
  the release build; the debug build is only a smoke bound.
- The measurement uses `TestBackend`, so it covers wrapping, layout and buffer
  writes but not terminal I/O or the compositor.
- The cache is invalidated by revision, not by a structural diff; a single
  appended entry still rebuilds the full 200-entry projection. Incremental
  line diffing is future work if the cap or entry size grows.
