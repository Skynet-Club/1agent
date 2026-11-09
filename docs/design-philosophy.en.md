# Design Philosophy

> Ask one question before acting.  
> Make the purpose clear, then build the thing — and make it possible to prove why it counts as done.

[简体中文](design-philosophy.md) · **English**

1Agent starts from a few invariants, not from “connect more tools.”

## 1. Purpose before means

A user says “I need a drill.” They may actually need to hang a picture or run a cable. The right answer can be completely different once the purpose is clear.

1Agent treats requirement regression as a first-class capability:

- decide whether the request is a **goal** or a **means**;
- if the purpose is unclear, ask the one question that can change the plan;
- when proposing a substitute, explain why it is closer to the purpose;
- the human makes the final decision; the model does not make it for them.

This is why the project is called First Principles. It does not start with an answer; it starts by confirming the problem.

## 2. Evidence is a product primitive, not an appendix

Many agents say “done” at the end. 1Agent requires answers to these questions:

- What was done?
- What was it based on?
- Which step produced a side effect?
- How was the result verified?
- If the run is replayed, does it lead to the same conclusion?

Therefore:

- every run has an append-only journal;
- every tool call is paired with its observation;
- a completion claim must trace back to the journal, an Artifact or a receipt;
- exit code 0 does not mean verified;
- “done” without evidence is not done.

## 3. Humans are authorizers, not spectators

1Agent is **read-only by default**.

- Reading files and searching the workspace can happen by default;
- writing files, running commands and network access require explicit authority;
- irreversible actions require step-by-step confirmation;
- a DelegationProfile selects a working style; it does not grant more authority than the AuthorityProfile;
- a subagent or Worker cannot approve itself, widen its authority, or change Authority/Policy/Persona.

This is not friction for its own sake. Clear boundaries are what make delegation possible.

## 4. Local-first, portable data

The core of 1Agent does not require a cloud account:

- one runtime home: `$ONEAGENT_HOME` or `$HOME/.1a`;
- memory, journals, skills and goals/commitments live locally;
- export bundles include a SHA-256 manifest and exclude key material by default;
- restore, upgrade, rollback and uninstall have explicit paths;
- keys and configuration are separate; credentials are referenced through environment variables or local key files.

Local-first does not mean rejecting the cloud. It means the user decides where data goes.

## 5. Provider-neutral, visible routing

1Agent does not put one model vendor in the domain core.

- a deterministic provider for offline tests;
- an OpenAI-compatible adapter for DeepSeek, Kimi, Qwen, Ollama and similar endpoints;
- routing filters by capability, risk, cost, latency and data boundary;
- each run can explain which provider/model was used, why alternatives were rejected, and what it cost.

Models can change; the behavior and the evidence contract cannot.

## 6. One agent identity, multiple executors

1Agent speaks with one voice.

- the Router/coordinator understands, dispatches and gives the final answer;
- Workers execute in isolated contexts and do not talk to the user directly;
- execution can be delegated to Workshop sessions, sandboxes and leases;
- audit records the relationship between parent task, subtasks, Workers and Artifacts;
- “multi-agent” is not a group chat; it needs ownership, budgets, termination conditions and a verifier.

## 7. Reversibility first

1Agent prefers designs that can be undone:

- soft and hard memory forget;
- skill versions, evaluation, canary and rollback;
- evolution proposals with evidence, approval, application, rollback and a ledger;
- approvals with one-time, standing, revoke and expiry semantics;
- reproducible builds and a release manifest.

Irreversible actions are not forbidden, but they need explicit confirmation, audit and a clear statement that approval is not the same as undo.

## 8. Simplicity first, conclusion first

The answer contract emphasizes:

- conclusion first, then evidence;
- keep only information that changes the decision;
- separate facts, assumptions, unknowns, conflicts and flip conditions;
- do not confuse a long answer with a good answer;
- do not replace verifiable results with model self-report.

## 9. Open source, without overclaiming

1Agent is MIT-licensed. Open source means the code can be inspected, replaced and run locally. It also means the boundaries must be clear:

- this is an R1 release candidate, not production-stable;
- Linux x86_64 is the validated platform;
- browser, generic subagents, complete always-on daemon and Bot are unfinished;
- without a third-party audit, we do not claim organization-level security certification;
- without a real long-horizon task, we do not claim full autonomy.

## 10. What the trade-offs mean

| Choice | Benefit | Cost |
|---|---|---|
| Evidence first | Explainable, replayable, auditable | More records and heavier implementation |
| Read-only default | Less accidental damage and privilege misuse | First use needs authorization setup |
| Provider-neutral | No single-model lock-in | More complex routing and evaluation |
| Local-first | User-controlled, portable data | Cloud sync and multi-device UX need extra work |
| One bounded identity | Predictable behavior and clear responsibility | Multi-agent throughput needs more scheduling |
| Reversibility | Errors can be rolled back | Versions, evaluation and a ledger are required |

These costs are deliberate.  
The goal is not to look the most human. The goal is to be worth trusting with something important.

---

[Back to README](../README.en.md) · [Features](features.en.md) · [Manual](manual.en.md)
