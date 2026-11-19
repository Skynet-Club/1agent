# 1ASH v1 schemas (M0)

Status: **draft schemas + examples; no runner yet**

- `task.schema.json` — task package (`1ash.task/v1`).
- `run.schema.json` — subject/runner/judge configuration (`1ash.run/v1`).
- `trace.schema.json` — one JSONL canonical trace event (`1ash.trace/v1`).
- `report.schema.json` — signed evidence report (`1ash.report/v1`).
- `examples/` — one valid example per schema.

The schemas use JSON Schema draft 2020-12. `task.schema.json` requires two
reviewers and a no-op baseline; `report.schema.json` requires metrics, cost,
confidence intervals, limitations and the ABC checklist object. These are
design gates, not proof that the 1ASH runner exists.

Structural check (hermetic, runs in `cargo test`):

```bash
cargo test --offline --test 1ash_schema
```

Full JSON Schema validation (optional Python dependency):

```bash
python3 scripts/check-1ash-schemas.py
```
