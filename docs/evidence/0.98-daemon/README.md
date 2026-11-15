# 0.98 Daemon / Durable Scheduling Evidence

Date: 2026-11-15  
Gate: G5 (ledger, crash matrix, watchdog)

## Reproduce

```bash
cargo test --offline --lib schedule_ledger
cargo test --offline --test ledger_crash
cargo test --offline --test watchdog_cli
```

Raw output: `tests.txt`.

## Result

- `RunLedger::claim` fsyncs before spawn; duplicate slots return
  `AlreadyClaimed`;
- `reconcile` marks claimed/running records interrupted and does not retry;
- 100-slot claim/reconcile matrix has no duplicates;
- 100 real child processes claim a slot then `_exit(137)`; every restart
  reconciles and cannot re-claim the slot (0 duplicate runs);
- systemd user unit is hardened and never enables linger;
- `sd_notify` READY/WATCHDOG is sent through the filesystem socket;
- watchdog tests cover stale heartbeat, pause-once and no auto-resume;
- deterministic jitter and skip-if-late / catch-up-latest are implemented.

## Known gaps

- bounded/all catch-up variants are not implemented;
- tool-level destination idempotency is not wired;
- no multi-day systemd soak in this evidence set.
