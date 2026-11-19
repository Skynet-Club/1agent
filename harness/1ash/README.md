# 1ASH local harness (M1–M4)

Status: **local synthetic harness**. It runs task packages through Inspect and
scores the final workspace; the 30-task suite and safety calibration are
synthetic, not human-reviewed and not a benchmark result.

Entry points:

```bash
harness/1ash/1ash.py m1       # M1 fake/no-op + trace
harness/1ash/1ash.py suite    # 30 synthetic tasks
harness/1ash/1ash.py safety   # synthetic injection calibration
harness/1ash/1ash.py verify REPORT
harness/1ash/1ash.py pilot 0.1.0
harness/1ash/1ash.py docker   # requires Docker + build_docker.sh
```

- `adapter.py` — Inspect `Task`/`Solver`/`Scorer`; subject protocol:
  `--task TEXT --workspace DIR --journal-dir DIR`; projects the subject
  journal to `.1ash/trace.jsonl`.
- `subject_1agent.sh` — real subject wrapper around `1agent run`.
- `tests/` — deterministic fake/no-op subjects, two M1 task packages and a
  synthetic journal fixture.
- `tests/run_m1.py` — Inspect-level check: fake subject 1.0, no-op 0.0, trace
  validates against the Trace IR schema.
- `report_v0.py` / `verify_report.py` — generate a local report v0 from one
  eval log and verify its schema plus hash-chain tip. This is evidence, not a
  benchmark result.
- `suite/` — 30 synthetic six-family tasks, reference subject, no-op and
  safety calibration; `suite/build_suite.py` regenerates them.
- `Dockerfile` / `build_docker.sh` / `tests/run_m1_docker.py` — network-off
  fake/no-op sandbox image. The real provider subject needs network and is not
  claimed as sandboxed.
- `make_pilot.sh` / `PILOT.md` — local pilot tarball; external rerun is not
  included or claimed.

Run the deterministic check (Inspect in an isolated venv):

```bash
~/.cache/1ash-venv/bin/python harness/1ash/tests/run_m1.py
```

Run the real 1Agent subject (needs `~/.1a/config.json`; override with
`ONEASH_1AGENT_CONFIG`). Inspect changes its working directory, so pass
absolute paths:

```bash
~/.cache/1ash-venv/bin/inspect eval "$PWD/harness/1ash/adapter.py@one_ash_task" \
  -T task_file="$PWD/harness/1ash/tests/task-command.json" \
  -T subject="$PWD/harness/1ash/subject_1agent.sh" \
  --model mockllm/model --display none --log-dir /tmp/1ash-eval --log-format json
```

Generate and verify a report v0 from that eval log:

```bash
~/.cache/1ash-venv/bin/python harness/1ash/report_v0.py \
  --task harness/1ash/tests/task-command.json \
  --eval-log /tmp/1ash-eval/<log>.json \
  --agent-binary target/debug/first-principles-agent \
  --out /tmp/1ash-report.json
~/.cache/1ash-venv/bin/python harness/1ash/verify_report.py /tmp/1ash-report.json
```

M1 limits: local workspace only, no container/network isolation, safety gates
are rejected rather than enforced, no report signing, and the report is a
single synthetic task. The wrapper copies the selected provider config into the
run journal and raises `max_risk` to `high` plus `allow_network` so the coding
subject can run; credentials stay referenced by path and are not copied.
