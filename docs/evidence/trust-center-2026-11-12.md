# Trust Disclosure Manifest — Evidence (2026-11-12)

Status: **passed on this machine**  
Scope: `trust_center::build_disclosure` and `1agent trust-center`.

## Commands

```bash
cargo test trust_center --lib
cargo test --test trust_center_cli
cargo clippy --all-targets -- -D warnings
```

## Expected results

```text
running 8 tests
test trust_center::tests::disclosure_counts_capabilities_without_overclaiming_audit ... ok
test trust_center::tests::disclosure_round_trips_through_json ... ok
test trust_center::tests::evaluations_are_validated_and_sorted ... ok
test trust_center::tests::html_escapes_caller_supplied_text ... ok
test trust_center::tests::incidents_are_validated_sorted_and_rendered ... ok
test trust_center::tests::markdown_states_the_audit_and_limits ... ok
test trust_center::tests::verification_summary_is_included_when_supplied ... ok
test http_surface::tests::handler_serves_the_trust_center_without_bearer ... ok

test result: ok. 8 passed; 0 failed

running 6 tests (tests/trust_center_cli.rs)
test trust_center_accepts_and_escapes_incident_reports ... ok
test trust_center_includes_a_verification_summary ... ok
test trust_center_rejects_an_inconsistent_incident_report ... ok
test trust_center_rejects_invalid_evaluations ... ok
test trust_center_writes_a_disclosure_with_evaluations ... ok
test trust_center_writes_escaped_static_html ... ok

test result: ok. 6 passed; 0 failed
```

## What this proves

- Capability counts sum to the Agent Card total and include verified entries.
- Audit is always `not_audited` with `external_audit: false`.
- Data policy reports no telemetry.
- Evaluations with `passed > total` or a malformed digest are rejected.
- The CLI writes the same JSON it prints, including capability counts,
  evaluations, contact and incident URL.
- With `--verification-store`, the disclosure includes a local all-time
  verified/hacked/rejected/bps summary.
- `--markdown-out` writes a Markdown page containing the version, audit
  status, limits, evaluation result and contact.
- `--html-out` writes a static page whose caller-supplied contact
  (`<script>alert(1)</script>`) and incident URL (`javascript:alert(1)`) are
  escaped and rendered as text; no script tag or `javascript:` href appears.
- `--incident-report` accepts a resolved incident and renders it newest first
  in JSON, Markdown and HTML; the summary's `<script>` is escaped and the
  incident link is text, not an `href`.
- Duplicate incident ids, an open incident with `resolved_at_ms`, a resolved
  incident without a valid resolution time, unsafe ids and non-http(s) links
  are rejected.

## What this does not prove

- That a supplied evaluation actually ran or passed.
- Third-party audit, SOC2, ISO 42001 or EU AI Act conformance.
- That a supplied incident report is true, complete or independently verified;
  there is no hosted incident feed or public availability. The HTML output is
  a local static file.
- Per-deployment data-policy enforcement.
- Cross-platform behavior; tests ran on Linux only.

## Files

- `src/trust_center.rs`
- `src/main.rs` (`trust-center`)
- `tests/trust_center_cli.rs`
- `docs/adr/0076-trust-disclosure-manifest.md`
- `docs/architecture.md`, `docs/features.md`, `docs/features.en.md`,
  `docs/claims.md`, `CHANGELOG.md`
