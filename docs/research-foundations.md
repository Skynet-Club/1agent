# Research Foundations

Date: 2026-11-02

Status: design reference, not a claim of exhaustive literature review

## Method

Sources were evaluated for mechanisms relevant to evidence-accountable decision agents. A paper or
repository is not adopted because it is popular; it is adopted only when its mechanism maps to a
testable contract, control or evaluation.

The current environment does not support a fresh exhaustive literature search. This document does
not claim coverage of every 2025–2026 publication.

## Paper and benchmark directions

| Direction | Representative work | Adopted mechanism | Limit |
|---|---|---|---|
| Agent architecture | CoALA, ReAct, ReWOO | decision loop, tool observation and explicit working state | loops can stall; plans can propagate errors |
| Planning/search | Tree of Thoughts, Plan-and-Solve, LLM+P, PAL | branch/search only with an evaluator; use external solvers for exact tasks | verbose plans and LLM arithmetic are unreliable |
| Reflection | Reflexion, Self-Refine, CRITIC | revise after external feedback or validation | self-critique without new evidence can reinforce error |
| Memory | Generative Agents, MemGPT, Lost in the Middle | scoped retrieval, relevance and provenance | simulation and long-context results do not prove production memory safety |
| Tools/interfaces | Toolformer, SWE-agent | structured calls and model-specific computer interfaces | model strength cannot compensate for bad action ergonomics |
| Multi-agent | Multiagent Debate, MetaGPT, failure studies | independent checks where value is demonstrated | coordination and specification failures can dominate |
| End-to-end evaluation | AgentBench, WebArena, SWE-bench, GAIA, OSWorld, τ-bench | environment outcomes, repeatability, cost and stability | one benchmark or one run is not product evidence |
| Safety | AgentDojo, ToolEmu | retrieved content is untrusted; least authority and sandboxing | instruction-following does not replace isolation |

## Open-source engineering references

Reviewed locally or as public engineering references:

- NousResearch `hermes-agent`;
- deepseek-ai `deepseek-harness`;
- HumanLayer `12-factor-agents`;
- OpenAI Agents SDK;
- LangGraph;
- PydanticAI;
- AutoGen;
- smolagents;
- SWE-agent.

| Reference | Adopted lesson | Rejected overreach |
|---|---|---|
| 12-factor-agents | own prompts, context and control flow; structured tools; durable state | copying a framework is not architecture |
| DeepSeek Harness | event log as fact source; model-visible means recorded; capabilities as seams | do not bind domain contracts to one provider |
| Hermes Agent | provider switching, tool filtering, approvals, secret hygiene, procedural experience | do not infer safe autonomy from feature breadth |
| OpenAI Agents SDK | small agent/guardrail/session/tracing primitives | handoffs are not the default answer |
| LangGraph | explicit state, checkpoints and human interrupts | graph execution is unnecessary for every request |
| PydanticAI | typed dependencies and validated outputs | retries cannot replace contract validation |
| AutoGen/MetaGPT/CrewAI | role decomposition and SOPs as research patterns | role-play is not proof of reliability |
| smolagents | code as an action substrate | code execution requires a sandbox and authority model |
| SWE-agent | optimize the agent-computer interface | raw shell exposure is not an interface design |

## Cross-source conclusions

1. Prefer the least autonomy that can complete the task.
2. Use deterministic workflows where they suffice.
3. Treat external observations as stronger evidence than model self-report.
4. Use external solvers for exact computation and constraint satisfaction.
5. Keep memory scoped, versioned, expiring and provenance-bound.
6. Start with one agent plus tools; require evidence for multi-agent complexity.
7. Audit plans, actions, observations, evidence and stop reasons, not hidden reasoning text.
8. Own the control flow, tool contract and message semantics.
9. Persist enough state for interruption, resume and human review.
10. Evaluate repeated behavior, tail latency, cost and safety, not just average answer quality.

## Design mapping

| Principle | Repository mechanism |
|---|---|
| evidence accountability | claim/evidence provenance validation and journal audit |
| result-first behavior | answer schema v2 and system prompt |
| uncertainty honesty | conditional/needs-evidence status, unknowns and flip conditions |
| provider neutrality | canonical model contracts plus adapters |
| recoverability | JSONL journal, checkpoints and route-bound resume |
| controlled evolution | L2 experience lifecycle with held-out verification |
| report trust | policy/integrity/digest/HMAC and explicit trust states |
| evaluation realism | contract suites separated from model-quality suites |

## Remaining research questions

- Which task families benefit from search rather than direct execution?
- Does structured decision completeness improve externally judged correctness?
- When does experience improve held-out performance without increasing overconfidence?
- Which provider differences require capability-specific routing?
- How should reviewer calibration and confidence calibration be combined?
- When is multi-agent independence worth its coordination cost?

These require real-task evidence, not another architectural metaphor.
