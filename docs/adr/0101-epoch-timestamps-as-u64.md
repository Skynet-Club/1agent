# ADR 0101: Epoch-Millisecond Fields Use u64

- Status: accepted
- Date: 2026-11-12

## Context

`serde` buffers the payload of internally/adjacently tagged enums before it
deserializes the selected variant. Its buffered `Content` representation does
not support `u128`. Journals whose tagged events contained a `u128` field
therefore failed to read with `u128 is not supported`, even though the number
itself fit in `u64`.

The concrete failures were:

- `ModelResponded` → `AgentAnswer` → `Evidence.observed_unix_ms` /
  `expires_unix_ms` (`Option<u128>`);
- `ReframeDecided.decided_at_unix_ms` (`u128`).

Top-level `JournalEvent.timestamp_unix_ms` is a plain struct field and was
not affected.

## Decision

Epoch-millisecond fields use `u64`:

- `Evidence.observed_unix_ms` and `expires_unix_ms` are `Option<u64>`;
- `JournalEventKind::ReframeDecided.decided_at_unix_ms` is `u64`, and the
  replayed field is `Option<u64>`.

Call sites that need the wider application clock cast explicitly
(`u128::from`) or saturate (`u64::try_from(...).unwrap_or(u64::MAX)`). The
JSON wire format does not change: epoch milliseconds are integers either way.
No data migration is needed, and existing journals with the affected events
become readable again.

## Consequences

- Legacy and new journals with timestamped evidence or reframe decisions can
  be read, replayed, audited and included in `1agent metrics`.
- `u64` milliseconds cover beyond the year 500,000,000; this is not a
  practical range limit.
- Other `u128` fields remain in application structs (leases, memory, goals,
  inbox). If one is ever placed inside a tagged enum payload, it must be
  converted or given a custom deserializer.
- A regression test appends a `ModelResponded` with evidence timestamps and a
  `ReframeDecided` event, then reads the journal back.
