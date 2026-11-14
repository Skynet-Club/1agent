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
| Clean-source package gate | passed |

## Verified clean release

```text
source commit: e1adddd244ae0cb2dab3f417c414803d9b2d92ea
git dirty: false

Linux binary:
126b60dd40dfad308088738ac20b290dc3107b00c9f275dde905b7f2e3831bd1

Windows binary:
e2e7bba29d59ee053342e800e12cf82d8f75206c9a210358b4e94b9007a715f0

Linux tar.gz:
3741f2d408d41e648c79b31938dde38b559fee9690a49f8b2cfba37a2b0b0739

Windows zip:
aad282211d001ee480ac100ad1293fb0a07cf44572c53093f938d34fe41311d9

Release manifest:
98dbe3e4cab863a2fc53fe168ca340f079641d917c246b70792c0f950677f64d
```

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
