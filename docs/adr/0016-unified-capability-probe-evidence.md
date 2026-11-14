# ADR 0016: Unified Capability Probe Evidence

Status: accepted

Date: 2026-11-03

## Context

The Master-Agent V1 plan requires one task set across Pi, Hermes, Claude and Codex.
Previous evidence used ad hoc commands and could not be reproduced from a versioned
manifest. Raw runtime output also risks persisting credentials or prompt content.

## Decision

Add a `master capability-probe` runner with a versioned manifest:

- bounded tasks with prompts, timeouts and exact/contains/JSON-object success rules;
- bounded runtime commands without a shell;
- prompt delivery by argument, stdin or none;
- explicit blocked-output patterns for authentication and network failures;
- separate authentication, network, filesystem and timeout blocked classifications;
- per-run status `passed`, `failed` or `blocked`;
- normalized failure kinds;
- bounded stdout/stderr consumption;
- report only stdout/stderr byte counts and SHA-256 digests, never raw output;
- report manifest digest, command digest, agent version and OS/architecture.

The checked-in example manifest uses one task set for Pi, Hermes, Claude, Codex and the
1A-native DeepSeek route. Runtime-specific JSON-path rules let the DeepSeek route validate
`RunResult.answer.conclusion` without changing the shared prompts.
Running it is an explicit, potentially billable external action. The runner itself does
not add provider credentials or infer success from a declared capability.

Workshop read evidence and external runtime measurements remain separate gates. File-system
or authentication failures are recorded as blocked rather than converted to zero success
or hidden.

## Consequences

- Multi-runtime comparisons become reproducible and content-bound.
- Raw output, secrets and prompt content are not retained in probe reports.
- Timeouts, spawn failures, wrong exits and wrong outputs are distinct from blocked runs.
- Statistics must use measured `passed` results only; `blocked` is not a failed task result.
- A user must separately approve potentially billable real-runtime execution.

## Verification

- a fake passing runtime reports `passed`;
- wrong output reports `failed`;
- authentication output reports `blocked_authentication`;
- read-only app-server startup reports `blocked_filesystem`;
- command timeout and non-zero exit are represented explicitly;
- reports contain hashes and sizes but not raw output;
- CLI E2E writes a report with a passing and blocked runtime;
- the example manifest validates and contains no credentials.
- `--runtime` filters a shared suite without rerunning already measured runtimes.
- DeepSeek's JSON-path success rules are tested against nested `RunResult` output.
