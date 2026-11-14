# ADR 0098: Answer/Audit Layering

- Status: accepted
- Date: 2026-11-12

## Context

The default CLI answer printed every structured field and the route trace.
That made a simple result hard to scan and buried the conclusion under
claims, evidence, assumptions, constraints, unknowns and routing detail.
SA97-30 asks for the conclusion plus one trust-summary line by default, with
structured fields behind an explicit expand action, while high-risk answers
keep the evidence and flip conditions inline.

## Decision

1. `render_answer_default` prints status, conclusion, and one trust-summary
   line with counts for claims, evidence, assumptions, unknowns and flip
   conditions.
2. A high-risk answer inlines up to three evidence entries and all flip
   conditions. High risk means status `conditional` or `needs_evidence`, or
   any unknown with high impact. This keeps the decision-changing details
   visible without expanding everything.
3. `--details` on `run` and `resume` prints the full structured answer and the
   route trace. JSON output is unchanged.
4. Other callers (chat bridge, master and internal projections) keep the full
   `render_answer` output; only the interactive CLI default changes.

## Consequences

- A default run is scannable: conclusion, one summary line, and only the
  high-risk evidence/flip conditions.
- Nothing is lost: `--details` and the journal still expose every field and the
  route trace.
- The high-risk predicate is a policy heuristic, not a risk model. A
  low-risk answer with no high-impact unknown still hides its evidence by
  default; the summary line tells the operator that more exists.
- `--details` is a presentation switch. It does not change what was journaled,
  validated or authorized.
