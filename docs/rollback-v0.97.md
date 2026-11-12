# 0.97 Rollback and Known Limitations

- Status: **draft for the 0.97 release gate**
- Date: 2026-11-12
- Applies to: local CLI/TUI, daemon/worker, journals, config, skills, memory

0.97 is not released yet. This page defines how to roll back a local build and
what the release must not claim.

## 1. Binary rollback

1. Keep the previous release directory (for example
   `releases/1agent-v0.96.1/`) and its `SHA256SUMS`.
2. Record the current binary digest before upgrading:
   `sha256sum 1agent`.
3. To roll back, stop the daemon/worker, replace the binary with the previous
   digest-verified file, and restart with the same config.
4. `1agent --version` and the release notes identify the running version; do
   not infer it from a filename.

## 2. Journal/schema rollback

- New journal events are additive: `CheckpointDelta` and `StreamFirstToken`.
- **An older binary cannot read a journal containing `checkpoint_delta`.** It
  fails on the unknown event variant. Rolling back the binary requires
  archiving or moving aside journals written by 0.97; do not point an older
  binary at them.
- `Evidence` timestamps and `ReframeDecided` now use `u64` on the wire; the
  JSON number format is unchanged, so older readers that understood `u128`
  still parse the same values.
- There is no destructive data migration in 0.97. Do not delete journals to
  “fix” a version mismatch; archive them with their SHA-256.

## 3. Config rollback

- Keep a copy of the previous config file and environment.
- Provider/model/base URL, budgets, permissions and route objective are the
  high-impact settings. Restore the file, then run `1agent doctor` and a
  deterministic `1agent run` before re-enabling the daemon.
- Secrets stay in the environment or 0600 files; never put them in a rollback
  archive or shell history.

## 4. Kill switch and stopping work

- `ops_guard` provides runtime cost/call/time limits and a manual kill switch.
- Cancel a run with Ctrl+C (CLI/TUI) or the configured cancellation path; the
  journal records `RunStopped { Cancelled }`.
- Stop the daemon before replacing the binary. Do not kill a worker process
  mid-write; journals are append-only but a half-written line is not a
  recoverable checkpoint.
- `1agent resume-status --journal PATH` reports whether a run has a usable
  checkpoint; `resume` refuses unpaired tool calls.

## 5. Incident procedure

1. Stop new work (cancel runs, pause the daemon).
2. Preserve the journal directory and config; record their SHA-256.
3. Run `1agent audit --journal PATH --json` and `1agent metrics --journal-dir DIR`.
4. For a side effect, check the tool receipt and the child journal before
   retrying; do not replay an ambiguous write.
5. Write a postmortem with the incident id, impact, timeline, root cause and
   follow-up owner.

## 6. Known limitations at the 0.97 gate

- No browser/CUA executor; CUA is policy only.
- Watchdog `pause`/`repair` are recommendations, not executed actions.
- Same-UID local processes are unrestricted unless capability policy is
  configured.
- Windows/macOS secret storage and ACLs are unverified; Linux is the only
  verified platform.
- No external security/compliance audit; no run-journal cryptographic hash
  chain.
- M1/M2/M3, M7 real-provider TTFT, M10 pricing, M11/M12, M14 real sessions,
  M19–M23 golden set are not measured to the release targets.
- `ModelRequested` is still a full per-step journal copy; TUI/daemon
  projections do not use the incremental cursor.
- Skill scanning is heuristic; it is not a malware sandbox.

## 7. Rollback verification

```bash
sha256sum 1agent
1agent --version
1agent doctor
1agent audit --journal PATH --json
1agent metrics --journal-dir DIR --json
./scripts/verify-claims.sh
```

A rollback is complete only when the previous binary runs, doctor is clean
(or its warnings are understood), and the preserved journals remain readable
by the version that wrote them.
