# ADR 0097: Actionable Error Recovery Hints

- Status: accepted
- Date: 2026-11-12

## Context

Provider, journal, route and resume failures surfaced only the error string.
The operator had to know whether to retry, switch provider, raise a timeout,
fix the journal directory or resume from a checkpoint. SA97-22 asks for
actionable suggestions on provider, tool, permission and context failures.

## Decision

1. Add `RuntimeError::recovery_hint`, a pure mapping from the error variant
   and provider error code to a short next step.
2. Provider hints are code-specific: authentication → check key/account,
   rate limit → wait or switch, timeout/network → check connectivity/proxy or
   raise `--timeout-seconds`, invalid response → retry or switch model,
   unsupported capability → choose another model, invalid configuration →
   check provider settings, response too large → reduce context/output.
3. Journal → check writability/space; cancelled → re-run or `resume`; resume →
   inspect `resume-status`; no route/route plan → check manifests and budgets;
   invalid output → retry or split the task; experience/reframe → local
   configuration or retry.
4. The `run` and `resume` CLI handlers print `hint: ...` to stderr before
   returning the original error. Hints do not change the error, retry count,
   permissions or stop reason.

## Consequences

- A failed CLI run ends with one concrete next step instead of only a stack of
  error context.
- The hint is advisory. A wrong hint does not grant authority and does not
  mask the original error, which remains the exit status and stderr content.
- The TUI does not render the hint yet; only the CLI prints it.
- Hints are static strings with no secrets and no provider-specific endpoints.
