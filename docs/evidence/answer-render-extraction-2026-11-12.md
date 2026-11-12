# Answer Render Extraction — Evidence (2026-11-12)

Status: **passed on this machine**
Scope: second RFC-0001/SA97-25 split slice.

## Change

- New `src/answer_render.rs` (245 lines, including tests) holds
  `render_answer`, `render_answer_default`, `answer_status_label`,
  `answer_impact_label` and `claim_status_label`.
- `src/main.rs` imports them; its size drops by about 90 lines.
- No output strings or command behavior changed.

## Commands and results

```text
cargo test answer_render --lib       -> 3 passed
cargo test --test cli_e2e default_run_output_is_concise_and_details_expands_it
                                     -> 1 passed
cargo clippy --all-targets -- -D warnings -> 0 warnings
```

## What this proves

- The concise default hides structured fields for a low-risk answer and the
  full renderer still includes them.
- A high-risk answer inlines evidence and flip conditions.
- The label mappings are stable.
- The CLI default/`--details` output is unchanged after the move.

## What this does not prove

- That `main.rs` is fully split; only the renderer moved.
- That route-trace rendering is extracted; it remains in `main.rs`.
- Anything about TUI rendering; the TUI has its own path.
