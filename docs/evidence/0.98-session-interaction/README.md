# 0.98 会话交互证据

Date: 2026-11-15  
Scope: session picker, follow-up queue/context, interrupt/resume

## What was exercised

### Session picker

- `←` on an empty composer opens the military-green modal.
- `↑/↓` selection refreshes the right preview automatically; `Enter` switch,
  `n` new session.
- `/` search, `Ctrl+R` rename, `Ctrl+D` archive with confirmation, `Ctrl+A`
  show archived.
- Switching restores the transcript from journals.

Captures: `picker.txt` (initial selection preview), `preview-auto.txt`
(selection change refreshes preview without Space), and `picker-multi.txt`
(two sessions) at PTY 100x32 with the deterministic provider.

### Follow-up

- During a run, typing works and `Enter` queues the message.
- When the run ends, the queued message is submitted automatically.
- The next request carries the recent-turn context facts.
- `queue-journals.txt` shows two session journals: the first task and the
  queued follow-up; the follow-up request includes the previous request and
  answer status.

### Pause / interrupt / resume

- First `Esc` while running pauses: status becomes `⏸ 已暂停`, the run future is
  no longer polled, and no terminal journal event is written.
- `Enter` resumes from the same in-memory point; the elapsed timer excludes the
  paused interval.
- A second `Esc` interrupts. `Ctrl+C` also interrupts.
- `pause.txt` shows the paused and resumed/completed screens.

### Agent-分队 (v1)

- `Ctrl+G` opens the `分队 Agents` tab; the title bar is
  `Tab：“会话 Sessions | 分队 Agents ”`; `Tab` switches.
- `spawn_agent` writes a local registry before/after the child process; rows show
  task, depth, elapsed, status. The preview reads the newest child journal.
- The registry is filtered by `root_pid`; v1 is read-only.
- `squad-panel.txt` captures the empty-state tab and the Sessions tab; 
  `squad-panel-populated.txt` captures a seeded running member with the child
  journal preview; `squad-run.txt` captures the same panel while the parent run
  is active.

### Interrupt / resume

- `Ctrl+C` cancels the run and preserves the journal.
- `Ctrl+R` creates a resume prefix ending at the last checkpoint and calls
  `runtime.resume`.
- If no checkpoint exists before cancellation, the UI says so instead of
  claiming a successful resume. `interrupt.txt` records this honest case.
- `resumable_prefix_stops_at_the_last_checkpoint` covers the prefix logic with
  a synthetic terminal journal.

## Commands

```bash
cargo test --offline --lib tui::tests::session_picker
cargo test --offline --lib tui::tests::queued_follow_up
cargo test --offline --bin first-principles-agent resumable_prefix
cargo test --offline --bin first-principles-agent session_context_facts
```

## Limits

- The mock provider's sleep happens before a checkpoint, so the recorded
  interrupt test cannot demonstrate a successful resume. A real provider run
  with a checkpoint after a tool call is still required.
- PTY captures use a deterministic provider; they prove interaction and
  journal behavior, not model quality. The populated Agent-分队 render is
  covered by `tui::tests::squad_tab_renders_running_member_and_status`; the
  PTY capture shows the empty state because no child was spawned in the mock
  run.
- Pause is UI-level (stop polling the run future). An already-running blocking
  tool or provider request may finish in the background; pause time still counts
  toward task/provider timeouts. A runtime-level pause state machine and
  background multi-session agent view remain P2.
- Agent-分队 v1 cannot stop/retry/attach to a child, the registry is not
  persisted into the session, and parent pause does not pause a running child.
- The child journal is read while it is being written; a partially written last
  line is ignored by the JSONL reader, so the preview may lag by one event.
