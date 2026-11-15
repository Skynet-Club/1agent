//! Real process-kill matrix for the durable scheduled-run ledger.
//!
//! Each iteration spawns this test binary as a child, claims a slot, and exits
//! hard with `_exit(137)` before any completion mark. The parent then restarts
//! the ledger, reconciles, and proves the same slot cannot be claimed again.

use std::process::{Command, Stdio};

use first_principles_agent::{ClaimOutcome, RunLedger, RunStatus};

const CHILD_ENV: &str = "LEDGER_CRASH_CHILD";

fn now_ms() -> u128 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_millis()
}

#[test]
fn ledger_child_claims_then_exits_hard() {
    let Some(path) = std::env::var_os(CHILD_ENV) else {
        return;
    };
    let schedule_id = std::env::var("LEDGER_CRASH_SLOT").unwrap();
    let scheduled_at_ms = std::env::var("LEDGER_CRASH_AT").unwrap().parse().unwrap();
    let run_id = std::env::var("LEDGER_CRASH_RUN").unwrap();
    let ledger = RunLedger::new(path);
    let outcome = ledger
        .claim(&schedule_id, scheduled_at_ms, &run_id, now_ms())
        .unwrap();
    assert!(matches!(outcome, ClaimOutcome::Claimed(_)));
    // Crash after the claim fsync, before Running/Completed is written.
    unsafe { libc::_exit(137) };
}

#[test]
fn one_hundred_hard_crashes_never_duplicate_a_claimed_slot() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("schedule-runs.jsonl");
    let exe = std::env::current_exe().unwrap();
    for index in 0..100u128 {
        let schedule_id = format!("job-{index}");
        let scheduled_at_ms = 1_000 + index;
        let child_run = format!("child-{index}");
        let status = Command::new(&exe)
            .args([
                "--exact",
                "ledger_child_claims_then_exits_hard",
                "--nocapture",
            ])
            .env(CHILD_ENV, &path)
            .env("LEDGER_CRASH_SLOT", &schedule_id)
            .env("LEDGER_CRASH_AT", scheduled_at_ms.to_string())
            .env("LEDGER_CRASH_RUN", &child_run)
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status()
            .unwrap();
        assert!(!status.success(), "child {index} did not crash");

        let ledger = RunLedger::new(&path);
        let interrupted = ledger.reconcile(now_ms()).unwrap();
        assert!(
            interrupted
                .iter()
                .any(|record| record.schedule_id == schedule_id
                    && record.scheduled_at_ms == scheduled_at_ms
                    && record.status == RunStatus::Interrupted),
            "reconcile missed {schedule_id}"
        );
        // A restarted daemon attempts the same slot. The fsynced claim must
        // win; no second run may start.
        let outcome = ledger
            .claim(&schedule_id, scheduled_at_ms, "parent-run", now_ms())
            .unwrap();
        let ClaimOutcome::AlreadyClaimed(record) = outcome else {
            panic!("slot {schedule_id} was claimed twice");
        };
        assert_eq!(record.run_id, child_run);
        let latest = ledger.latest().unwrap();
        let record = latest.get(&(schedule_id, scheduled_at_ms)).unwrap();
        assert_eq!(record.status, RunStatus::Interrupted);
        assert_eq!(record.run_id, child_run);
    }
    let latest = RunLedger::new(&path).latest().unwrap();
    assert_eq!(latest.len(), 100);
}
