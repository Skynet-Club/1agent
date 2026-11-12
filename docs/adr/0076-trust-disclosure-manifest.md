# ADR 0076: Trust Disclosure Manifest

- Status: accepted
- Date: 2026-11-12

## Context

SA99-65 asks for Trust Center v1: public evaluations, incident reports, audit
reports and data policy. None of the external inputs exist yet: there is no
third-party audit, no hosted page and no incident feed. A manifest that
invented those would be worse than no manifest.

## Decision

1. Add `TrustDisclosure` in `src/trust_center.rs` with:
   - schema version, generated time, product, version, platform;
   - capability counts derived from the Agent Card;
   - limits from the Agent Card;
   - an audit section fixed to `not_audited` and `external_audit: false`;
   - a data policy stating local storage, no telemetry, no default cloud
     dependency and network off by default;
   - caller-supplied evaluation disclosures, validated (total > 0,
     passed <= total, optional 64-hex evidence digest) and sorted by name;
   - an optional all-time local verification summary from
     `--verification-store` (verified/hacked/rejected/resolved/bps);
   - optional security contact and incident URL;
   - caller-supplied incident disclosures (id, title, status, opened time,
     optional resolved time, summary, optional http(s) link), validated for
     unique safe ids, non-empty bounded text, open/resolved timestamp
     consistency and escaped rendering; sorted newest first.
2. Add `1agent trust-center [--evaluation PATH ...] [--security-contact ...]
   [--incident-url ...] [--incident-report PATH ...] [--out PATH]
   [--markdown-out PATH] [--html-out PATH] [--json]`. `--markdown-out`
   renders the same disclosure as a human-readable page; `--html-out`
   renders a static, script-free page with every caller-supplied value
   HTML-escaped and contact/incident URLs kept as text.
3. Evaluations must be supplied by the caller from real runs. The command does
   not run an evaluation or read a hidden benchmark.
4. `1agent eval --disclosure-out PATH` writes a compatible disclosure for the
   run: suite name, package version, passed/total and the SHA-256 of the
   pretty-printed report. `trust-center --evaluation` accepts that file
   directly.
5. This is a JSON manifest. The HTTP Surface also serves the baseline
   disclosure at `GET /v0/trust-center` and its static HTML at
   `GET /v0/trust-center.html`; caller-supplied evaluations remain CLI-file
   inputs. It is not a public hosted Trust Center, not an external audit and
   not a compliance certification.

## Consequences

- A public disclosure can be generated reproducibly from the Agent Card plus
  supplied evidence.
- The audit field cannot accidentally overclaim; changing it requires a code
  change and evidence.
- Evaluation and incident files can be wrong; the manifest records what it
  was given and does not independently verify either.
- Audit reports, hosting and certification mappings remain future work; the
  HTML output is a local static file, not a hosted page.
- The data-policy strings are static claims about defaults, not per-deployment
  verification.

## Evidence

```bash
cargo test trust_center --lib
cargo test --test trust_center_cli
```

The tests cover capability counting, audit non-overclaiming, evaluation and
incident validation/sorting, JSON round-trip, Markdown/HTML output-file
writing, HTML escaping of contacts, incident text and links, and invalid
input.
