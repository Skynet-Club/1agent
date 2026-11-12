# Metrics Gate Policy — Evidence (2026-11-12)

Status: **passed on this machine**
Scope: fail-closed threshold evaluation and the release-check hook.

## Commands

```bash
cargo test --offline metrics --lib
cargo test --offline --test metrics_cli
./scripts/gate-check.sh <journal-dir>
```

## End-to-end check

A local `Research @read-file` run produced one completed journal:

```text
journals=1 runs=1 completed=1 interrupted=0 resumes=0
crash_free=100.00% recovery=- replay=100.00% compacted_replay=- tool_success=100.00% audit_complete=100.00% denied=0
```

- With `GATE_MIN_RECOVERY_BPS=0 GATE_MIN_COMPACTED_BPS=0`, the gate passes
  because replay and audit are 100%; the zero thresholds explicitly disable
  the two stages that have no samples.
- With the default thresholds, the gate exits non-zero and reports:

```text
gate violation: recovery_bps observed=None required=9500 direction=Minimum
gate violation: compacted_replay_bps observed=None required=10000 direction=Minimum
```

- A threshold above 10,000 bps is rejected before evaluation.

## What this proves

- A missing minimum sample fails closed; it is not treated as zero or as a pass.
- `0` disables a threshold, so operators can run a partial gate without a
  sentinel sample.
- The CLI exits non-zero on violations while leaving the JSON report shape
  unchanged.
- `release-check.sh --metrics-dir DIR` invokes the same gate check.

## What this does not prove

- That the supplied journal directory is representative or complete.
- M6 security incidents; run the security suite separately.
- Real-provider or cross-platform behavior.
- That `release-check.sh` passed in full; the local CI stage is still subject
  to the skynet-term session limit.
