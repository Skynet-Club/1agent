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
| Clean-source package gate | passed |

## Release artifacts

Source commit: `34ae92b75a9fa4e7b778568363923348e499e44f`

Clean-source artifacts were rebuilt after the feature commit. SHA-256:

| Artifact | SHA-256 |
|---|---|
| Linux `tar.gz` | `f4488a9972e8214c962e5066385ed1aaab8ace5570a02f5205495348bc219174` |
| Windows `zip` | `563e102e1edf32e58d2d5fbefb1e753e24c50c068e2a2468f31e81032dec9529` |
| Release manifest | `b470b5cfff97d56d678e2cae1f97ce8e55ad13a8f667d98b03b45de439e22b88` |
| Linux binary | `e7b0565c0f1bb6dec2fc124b759e544aad35485f14c1225b979e77092419c67d` |
| Windows binary | `5a9c52b519b5f574b30801724284fff035e7b334334a65a3498671841ae77fdf` |

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
