# Trust Center Audit Reports — Evidence (2026-11-12)

Status: **passed on this machine**
Scope: `AuditReportDisclosure`, validation, rendering and CLI.

## Commands

```bash
cargo test trust_center --lib
cargo test --test trust_center_cli
```

## What this proves

- A completed audit report with `completed_at_ms >= started_at_ms`, bounded
  text, P0/P1 counts and an http(s) link is accepted and disclosed.
- Completed without a completion time, completed before started, or
  planned/in-progress with a completion time fail closed.
- Bad links and duplicate ids fail closed.
- Reports sort newest-started first.
- Markdown and HTML escape caller text (`<script>` becomes `&lt;script&gt;`).
- The fixed `audit.status` remains `not_audited` even when a completed report
  is supplied.

## What this does not prove

- That any third-party audit occurred; the report is caller-supplied.
- That P0/P1 counts are accurate or independently reproduced.
- That the Trust Center is a hosted page or signed disclosure.
- That a link is reachable or trustworthy.
