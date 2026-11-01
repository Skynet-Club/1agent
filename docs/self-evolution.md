# Self-Evolution Boundary

Date: 2026-11-02

Status: L2 gated candidate evolution implemented; real held-out transfer pending

## Scope

Hermes-style evolution is adopted as controlled experience accumulation, not self-modifying code
or automatic policy rewriting.

The runtime may propose:

- memory candidates;
- procedural skill candidates;
- route-policy candidates.

Candidates do not change active behavior until verified, reviewed, approved and activated.

## Evolution levels

| Level | Behavior | V0 status |
|---|---|---|
| L0 | no persistent learning | baseline |
| L1 | draft candidates with no runtime effect | implemented |
| L2 | verified, approved, active experience with rollback | implemented |
| L3 | automatic code/prompt/policy modification | not implemented |

## Candidate shape

A candidate includes:

- kind;
- title and bounded content;
- applicability scope;
- source run ID;
- evidence-event sequence IDs;
- canonical evidence digest;
- sensitivity marker;
- applicable cognitive modes;
- optional expiration;
- optional supersession target;
- version and lifecycle status;
- verification report.

Text, scope, evidence-reference count, mode list and JSONL event size are bounded.

## Evidence rules

Only these source event classes can support an experience candidate:

- tool observation;
- produced answer;
- provider failure.

Lifecycle and routing-only events are invalid evidence.

Evidence references must:

- exist in an audited source run;
- be unique;
- refer to the declared source run;
- reconstruct to the stored canonical digest.

Rewriting the source journal after proposal invalidates review, approval or activation.

## Lifecycle

```text
draft
  -> reviewed
  -> verified
  -> approved | rejected
  -> active
  -> degraded | superseded | rolled_back
```

Rules:

- review and approval require source evidence validation;
- approval requires a passing verification report;
- activation requires an approved, non-expired candidate;
- only one Active candidate may exist for a kind and scope;
- supersession atomically moves the previous Active candidate to Superseded;
- rollback preserves lifecycle history;
- expired Active experience is an audit failure.

## Verification

Baseline and learned-shadow runs use disjoint training and evaluation groups and the same held-out
suite.

Verification records:

- suite ID/version/digest;
- case IDs;
- baseline and learned success;
- baseline and learned unsupported-certainty rates;
- cost delta;
- evidence summary.

Promotion fails when:

- training and evaluation groups overlap;
- the suite is a contract fixture presented as model quality;
- learned success is not better;
- unsupported certainty increases;
- source evidence is missing or changed;
- the candidate is expired;
- the candidate duplicates an evidence sequence.

## Runtime retrieval

Retrieval filters by:

- Active status;
- cognitive mode;
- scope;
- expiration;
- approved source evidence.

Injected guidance is explicitly marked as approved but unverified policy, never as fact, and
cannot override current task constraints or evidence.

## Audit

Experience-store audit reports:

- invalid JSON/sequence/schema;
- oversized events;
- non-monotonic timestamps;
- invalid transitions and verification;
- invalid evidence or metadata;
- invalid supersession;
- status counts, chain depth and cycles;
- expired Active records;
- orphaned supersession references.

## Threat boundary

Candidates can encode wrong or stale guidance. Controls reduce risk but do not prove safety:

- content is untrusted guidance;
- scope is mandatory;
- sensitive marking does not replace redaction;
- a single suite can overfit;
- approval is a governance control, not empirical proof;
- HMAC evidence controls do not establish external truth.

## Evidence required before expanding autonomy

- reproducible positive transfer on real held-out tasks;
- no increase in unsupported certainty;
- stable benefit across multiple providers or declared provider scope;
- reviewer agreement on candidate applicability;
- rollback drill evidence;
- expiration and supersession audit over long histories.

Until then, L3 automatic self-modification remains out of scope.
