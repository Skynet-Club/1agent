# Status v0.52.0

Date: 2026-11-02
Status: evidence freshness policy implemented

## Changes since v0.51.0

- Evidence adds:
  - `observed_unix_ms`;
  - `expires_unix_ms`.
- Task requests add `evidence_max_age_ms`.
- Run CLI adds `--evidence-max-age-ms`.
- Verified claims reject evidence whose expiration has passed.
- When task freshness policy is enabled:
  - missing observation timestamp is rejected;
  - evidence older than the configured maximum age is rejected.
- Invalid observation/expiration windows are rejected.
- JSON scorecards report stale evidence count.

## Verification

| Check | Result |
|---|---|
| Missing observation under policy | rejected |
| Stale evidence under policy | rejected |
| Expired verified evidence | rejected |
| Fresh evidence | passed |
| Stale evidence count | implemented |
| Unit tests | 88 passed |
| Core evaluation | 6/6 passed |

## Known limitations

- Timestamps are caller/model supplied.
- No trusted timestamping or source clock validation exists.
- Freshness policy downgrades by rejection; automatic claim-status mutation is not performed.

## Next requirement

Version the answer evidence contract:

1. Add an explicit schema version to structured answers.
2. Keep decoding unversioned legacy answers.
3. Include schema version in journal audit.
4. Reject unsupported future versions.
5. Test migration compatibility.
