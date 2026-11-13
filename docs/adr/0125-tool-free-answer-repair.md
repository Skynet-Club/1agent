# ADR 0125: Tool-Free Answer Repair and Exact Evidence Sources

- Status: accepted
- Date: 2026-11-13
- Related: M1, M5, M13, SA97-31

## Context

The 8h benchmark failed at 48.57% all-suite completion. Diagnostic runs showed
three model-contract failures:

- the automatic answer-repair call returned tool calls instead of a final
  `AgentAnswer` JSON object;
- a completed answer relied on a high-impact assumption, and the repair did not
  produce a valid answer;
- `evidence.source` sometimes contained a prose description of a tool result
  instead of the concrete path, so the rubric could not match the required
  source.

The repair request previously reused the original tool list, so the model could
continue acting instead of repairing the answer.

## Decision

1. The answer-repair request is sent with an empty tool list. Repair is a
   text-to-JSON correction step, not another tool phase.
2. The repair instruction says that an answer depending on a high-impact
   assumption must use `conditional` or `needs_evidence`, not `completed`.
3. The system prompt requires `evidence.source` to be the concrete path itself,
   not a sentence describing a tool result.
4. Tests assert that both repair providers receive no tools and that the repair
   prompt carries the high-impact-assumption rule.

## Consequences

- The model cannot satisfy a repair by calling another tool; it must return a
  corrected JSON answer.
- An irreparable output still fails closed as `RuntimeError::InvalidOutput`.
  Converting that case into an auditable `needs_evidence` answer remains a
  separate decision.
- The prompt changes reduce, but do not prove, rubric source mismatches; a
  benchmark rerun is required before any release claim.
