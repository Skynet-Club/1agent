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
| Clean-source package gate | passed |

## Release artifacts

Source commit: `d89dcb4ab50a4094fb0b498712a6985b64be209b`

Clean-source artifacts were rebuilt after the feature commit. SHA-256:

| Artifact | SHA-256 |
|---|---|
| Linux `tar.gz` | `367c5181f8a6120cf910b209b8e6bbbd31f21f49009920e00eef70b249e0fe19` |
| Windows `zip` | `70c13d359307410bd161e4ca99e9ff8868415d5ddd90fc130e532fa72e9f62ac` |
| Release manifest | `ac71eb06f70bd9e24c7d4c8d637ae25198eb05caf727e07cd19b8c561d4d5053` |
| Linux binary | `bc32dd5de9dbb25677631ae5cdc1bd645c865b1a4175e755822e05ec20e2fbe7` |
| Windows binary | `a982e9f68322c145c9d6ad24e65023d29473cdcd603c18489925d7df94e5265a` |

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
