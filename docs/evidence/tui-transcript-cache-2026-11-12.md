# TUI Transcript Projection Cache — Evidence (2026-11-12)

Status: **passed on this machine**
Scope: transcript projection cache keyed by content revision and width; M15
frame-time probe.

## Commands

```bash
cargo test transcript_projection --lib
cargo test long_transcript_frame_budget --lib -- --ignored --nocapture
cargo test --release long_transcript_frame_budget --lib -- --ignored --nocapture
```

## Results

```text
transcript_projection_rebuilds_after_a_new_entry ... ok
transcript_projection_rebuilds_when_the_width_changes ... ok

tui frame p95=11.085399ms max=12.276598ms profile=debug
tui frame p95=1.258506ms max=2.243727ms profile=release
```

The probe renders a 200-entry transcript at 120x40, pushes a new entry every
fourth frame and measures 120 draws with `TestBackend`.

## What this proves

- Two unchanged frames keep the same projection revision; pushing a new entry
  bumps it and the next draw shows the new text.
- Resizing the terminal changes the cached width and rebuilds the projection.
- In a release build, the p95 frame time with a 200-entry transcript and
  periodic updates is 1.26 ms, below the M15 8 ms gate. The debug build is
  11.1 ms, so the probe uses a looser 25 ms debug smoke bound.

## What this does not prove

- Real terminal I/O, compositor or font rendering time; `TestBackend` only
  covers wrapping, layout and buffer writes.
- Frame time above 200 transcript entries, because the TUI caps the transcript
  at 200.
- That every panel is cached; only the transcript projection is.
- Incremental line diffing; each content change rebuilds the full projection.
