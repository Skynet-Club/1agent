# Status v0.77.0

Date: 2026-11-02
Status: fail-closed evaluation report inspection implemented

## Command

```bash
1agent eval-inspect --report report.json
1agent eval-inspect --report report.json --json
```

JSON mode emits the normalized `EvalReport`. A legacy report without policy
metadata is normalized to policy version `0` and source `legacy`.

## Validation policy

| Version | Source | Result |
|---|---|---|
| `0` | `legacy` | accepted |
| `1` | `default` | accepted |
| `1` | `explicit` | accepted |
| `0` or `1` | inconsistent source | rejected |
| other | any | rejected |

Unknown fields and unknown source values also fail decoding.

## Verification

| Check | Result |
|---|---|
| Unit tests | 122 passed |
| Integration / E2E | passed offline |
| Core evaluation | 6/6 passed |
| valid current policy | passed |
| valid legacy policy | passed |
| inconsistent combination | rejected |
| unsupported version | rejected |
| CLI JSON normalization | passed |
| Clippy `-D warnings` | passed |
| Clean-source package gate | pending |

## Known limitations

- Inspection validates policy provenance, not the truth of evaluation cases.
- The command does not recompute case results from journals.
- Report metadata remains unsigned.
- Unsupported versions fail rather than attempting best-effort interpretation.

## Next requirement

Bind report inspection to the report's own integrity fields:

1. validate total, passed, failed and case count consistency;
2. validate aggregate decision-completeness counts against case rollups;
3. reject contradictory floor status;
4. keep policy validation and legacy decoding unchanged.
