# Status v0.79.0

Date: 2026-11-02
Status: case-content-free evaluation report summary implemented

## Command

```bash
1agent eval-inspect --report report.json --summary
1agent eval-inspect --report report.json --summary --json
1agent eval-inspect --report report.json --json
```

The first two commands emit a bounded status summary. The last command preserves the
full normalized report contract.

## Summary fields

```text
suite
kind
total
passed
failed
hard_gate_failures
decision_completeness_cases
decision_completeness_ok_cases
decision_completeness_rate_bps
min_decision_completeness_bps
decision_completeness_floor_passed
decision_completeness_policy
```

The summary excludes case records, case IDs, issues, model output and evidence content.
It is generated only after report policy and integrity validation succeeds.

## Verification

| Check | Result |
|---|---|
| Unit tests | 123 passed |
| Integration / E2E | passed offline |
| Core evaluation | 6/6 passed |
| summary aggregate fidelity | passed |
| case payload exclusion | passed |
| full report compatibility | passed |
| integrity gate preserved | passed |
| Clippy `-D warnings` | passed |
| Clean-source package gate | pending |

## Known limitations

- The summary does not identify failing cases.
- Summary metadata is not signed.
- Integrity checks cover report arithmetic, not external truth.
- Full normalized JSON remains available and may contain case content.

## Next requirement

Version the report-summary contract and reject summaries whose policy metadata or numeric
bounds are unsafe:

1. add a summary schema version;
2. validate count and basis-point ranges;
3. preserve version-less summary decoding where unambiguous;
4. test supported, legacy and unsupported versions.
