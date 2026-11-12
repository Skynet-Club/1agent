# Answer/Audit Layering — Evidence (2026-11-12)

Status: **passed on this machine**
Scope: default `run`/`resume` human output and the `--details` switch.

## Commands

```bash
cargo test --test cli_e2e default_run_output_is_concise_and_details_expands_it
```

## What this proves

- Default human output contains status, conclusion, a trust-summary line with
  claim/evidence/assumption/unknown/flip counts, the `--details` hint and the
  journal path.
- A `needs_evidence` answer inlines its flip conditions and any available
  evidence (up to three), while the route trace and terminal reason stay out
  of the default output.
- `--details` restores the terminal reason, constraints/unknowns and the route
  trace.
- JSON output and the internal `render_answer` callers are unchanged.

## What this does not prove

- That the high-risk predicate matches every real risk; it is a heuristic
  based on answer status and high-impact unknowns.
- That low-risk answers should hide evidence; the summary line reports the
  count so the operator can expand.
- TUI rendering; the TUI already has `/details` and `/why` paths and was not
  changed here.
