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
running 5 tests
test trust_center::tests::disclosure_counts_capabilities_without_overclaiming_audit ... ok
test trust_center::tests::disclosure_round_trips_through_json ... ok
test trust_center::tests::evaluations_are_validated_and_sorted ... ok
test trust_center::tests::markdown_states_the_audit_and_limits ... ok
test trust_center::tests::verification_summary_is_included_when_supplied ... ok

test result: ok. 5 passed; 0 failed

running 3 tests (tests/trust_center_cli.rs)
test trust_center_includes_a_verification_summary ... ok
test trust_center_rejects_invalid_evaluations ... ok
test trust_center_writes_a_disclosure_with_evaluations ... ok

test result: ok. 3 passed; 0 failed
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

## What this does not prove

- That a supplied evaluation actually ran or passed.
- Third-party audit, SOC2, ISO 42001 or EU AI Act conformance.
- Incident reporting, a hosted page or public availability; the HTML output is
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
