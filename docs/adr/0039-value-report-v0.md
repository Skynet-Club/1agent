# ADR 0039: Value Report v0 — Outcomes and Cost, Not Invented Time Savings

- Status: accepted
- Date: 2026-11-11

## Context

A partner must prove value, but "hours saved" is easy to fabricate and hard to
verify. The runtime already has the facts needed for an honest first report:
run stop reasons, the usage ledger, tool calls/failures and alerts.

The product plan's value report wants time, tasks, risks and cost. v0 should
ship the facts it can defend, and leave the time estimate to a later measured
baseline.

## Decision

1. Add `src/value_report.rs` with `ValueReport`:
   - bounded window (`since_ms..until_ms`);
   - run counts by `StopReason`: completed, needs authorization, needs
     evidence, failed, cancelled, other;
   - journals, tokens, cost, cost per completed outcome;
   - tool calls and failures;
   - alerts and `pending_decisions = needs_authorization + alerts`.
2. The caller passes a `UsageLedger` scoped to the same journals as the run
   list. The report does not re-read journals or guess a usage window.
3. Add `1agent value-report --journal-dir ... [--alert-log ...] [--since-ms ...]
   [--until-ms ...] [--json]`. Default window is seven days.
4. v0 does **not** report "time saved" or a monetary value. It reports
   completed outcomes and cost per completed outcome; a time estimate requires
   a measured baseline and is deferred.
5. Cost is reported in micros as recorded by the ledger; zero-cost calls remain
   visible through the ledger and are not silently treated as free.

## Consequences

- The user gets a reproducible weekly view instead of a marketing number.
- A high run count with few completed outcomes is visible, not hidden by a
  single "tasks" total.
- `pending_decisions` gives the inbox/autonomy work a metric to connect to.
- A later value model can add measured time savings on top of this report
  without changing the fact base.

## Evidence

```bash
cargo test value_report --lib
cargo test --test value_report_cli
```

The unit tests cover outcome counts, cost per completed outcome, tool totals,
window filtering and the zero-completed case. The CLI test runs a real mock
journal and reads it back through `value-report`.
