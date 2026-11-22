# Session compaction, steering and core 7 (2026-11-22)

Goal: finish the three remaining P2 levers — compact long sessions, let the
user steer an active run, and cut the default tool surface to the smallest
useful set.

## Change

- **Session compaction:** before a run loads context, the un-summarized tail
  is checked. Above 20 context entries or 24k characters, the oldest turns are
  replaced by a deterministic extractive `SessionEntry::Summary`; the last 10
  entries stay verbatim. The original entries stay on disk. Reasoning entries
  are skipped by projection and do not hide an unanswered user turn.
- **Steering:** while a run is active, `/steer <text>` queues an interjection.
  The runtime drains the queue at the next model step and appends a
  `STEERING (user interjection during the run): …` user message. Plain Enter
  still queues a follow-up after the run. Unconsumed steering is requeued as a
  follow-up when the run ends.
- **Core 7:** the TUI advertises `read_file`, `write_file`,
  `terminal_create`, `terminal_exec`, `acceptance_gate`, plus virtual
  `describe_self` and `request_tools`. `edit_file`, `search_files`,
  `preview_file`, `browser`, `clarify` and the rest are request-only; the page
  delta already tells the model to request them.

## Live check

Installed binary `003e3319…`, real DeepSeek, TUI through tmux.

### Core 7 note

Workspace `/tmp/tui-final-batch`, journal `8e474ccd-…`.

Result: 7 advertised tools, step-0 system **3,234**, tool schema **1,113**,
fixed system+schema **4,347**, context **4,569**, input **4,812**. 7 model
calls / 6 tool calls, 15.0s, file `batch`, `completed`.

### Session compaction

Seeded the session with 30 historical user/assistant entries, then asked
`你记得历史用户 14 说过什么吗？` (journal `2ed160b1-…`).

Result: 39 session entries with 1 `summary`; the follow-up request has 18
messages, starts with a 992-character `[Session summary]`, and **still carries
the exact kept messages** `历史用户 14…` and `历史回答 14…`. The live run
caught an early projection bug where the kept tail before the summary was
dropped; the projection now uses `first_kept_index`, and the test asserts the
exact kept assistant message. Status `completed`.

### Steering

A page task on the same core-7 surface (journal `ac09ab38-…`). Two seconds in,
the user sent `/steer 页面主标题 h1 必须包含 STEERLIVE`.

Result: 19 model calls / 16 tool calls, 122.9s, `completed`. The first request
had 7 tools; the model requested `preview_file` and `browser` through
`request_tools`, then wrote, previewed, audited and accepted the page. The
steering text appears in the journal's later model request and deltas, and
`/tmp/tui-final-batch/index.html` (7,239 bytes) contains `STEERLIVE`.

| Metric | Core 10 | Core 7 |
|---|---|---|
| Advertised tools | 10 | **7** |
| Tool schema | 1,543 | **1,113** |
| Fixed system+schema | 4,952 | **4,347** |
| Context | 5,201 | **4,569** |
| Input | 5,530 | **4,812** |

Cumulative from the 38-tool baseline: fixed 11,914 → **4,347** (-64%), first
context 12,287 → **4,569** (-63%).

## Tests

- `long_sessions_compact_into_a_summary_and_keep_the_tail`: summary created,
  chained summaries include the previous summary, exact kept assistant message
  survives projection.
- `steer_command_bypasses_the_follow_up_queue_while_running`: `/steer` fills
  the steering buffer; plain Enter still fills the follow-up queue.
- `steering_is_injected_into_the_next_model_step`: a provider injects a
  steering message on its first call; the second model request contains it.
- `core_tools_reduce_the_advertised_surface`: 7 tools max, full schema at most
  1,200 tokens, edit/search/preview absent.
- Gates: lib 1431/0/4, bin suite exit 0, fmt, strict clippy, eval 8/8.

## Limits

- Compaction is deterministic and extractive, not a model-generated summary;
  it caps at 2,000 characters and keeps originals on disk.
- `/steer` is the explicit command; plain Enter remains a follow-up queue. The
  requeue path for steering that arrives after the final model step is covered
  by code and unit behavior, not by a live run.
- Core 7 makes edit/search/preview request-only. The page run took 19 model
  calls and 123s; fixed per-call cost fell, but page tasks pay for the extra
  request_tools step.
- One live run per feature; model output is nondeterministic.
