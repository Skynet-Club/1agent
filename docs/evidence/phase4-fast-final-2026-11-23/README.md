# Fast delivery vs Pi — intermediate (superseded)

Superseded by `../phase4-fast-final2-2026-11-23/`; kept for audit.

- Binary `72a3f024…` read `read_file` metadata's `path` field; observations
  only carry `relative_path`, so read-only cases had no derived source and the
  queue receipt said conditional: 24/24 neutral, 18/24 status, 3,997ms.
- The `relative_path` fallback fixed the six misses; see final2.
