# ADR 0110: Trust Center Audit Reports

- Status: accepted
- Date: 2026-11-12
- Related: SA99-65, SA100-68

## Context

The Trust Center could disclose evaluations, incidents and a fixed
`audit.status = not_audited`, but it had no place for a third-party audit
report. `1agent trust-center` therefore could not show an auditor, scope,
completion state or P0/P1 counts.

## Decision

1. Add `AuditReportDisclosure` with id, title, auditor, status
   (`planned|in_progress|completed`), scope, started/completed timestamps,
   P0/P1 counts, summary and optional link.
2. Add `TrustDisclosure.audit_reports` (omitted when empty). Existing
   `build_disclosure_with_incidents` keeps its signature; a new
   `build_disclosure_with_reports` accepts audit reports.
3. Validate: safe unique ids, bounded text fields without control characters,
   finding counts ≤10,000, a completed report needs `completed_at_ms >=
   started_at_ms`, planned/in-progress reports must not have a completion
   time, and links must be single http(s) URLs.
4. Sort newest-started first. Render a Markdown section and an escaped HTML
   table. `1agent trust-center --audit-report PATH` is repeatable.
5. The fixed `audit.status` remains `not_audited`; an audit report disclosing
   completion does not flip that field. A caller must not treat a
   caller-supplied report as an independent audit.

## Consequences

- A release can disclose an audit's auditor, scope, state and finding counts
  without editing code.
- The report is caller-supplied and unverified; the trust center is still not
  a hosted page or an external audit.
- HTML/Markdown escape caller text; links are rendered as text, not clickable
  HTML anchors.
- P0/P1 counts are numbers from the report, not independently reproduced.
