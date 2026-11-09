# 1Agent Product Brief

## What it is

1Agent is a local-first, evidence-accountable super-agent runtime. It plans and
executes tasks, remembers user context under explicit scopes, evaluates skills,
tracks goals and commitments, and makes every material decision reconstructable
from a journal.

The product is split into three layers:

- **1Agent**: the standalone agent runtime and local service host.
- **Workshop**: optional execution substrate for long-running, isolated work.
- **Bot**: persona and channel layer built on 1Agent; not required to use 1Agent.

## Who it is for

- Individuals who want an agent that can explain what it did and why.
- Teams integrating a local agent through a typed Surface API.
- Builders who need a dependable agent core before adding a Bot persona or
  multi-channel distribution.

## Core capabilities

- Deterministic task graph, ticket, mandate, approval and report lifecycle.
- Provider-neutral routing with deterministic, OpenAI-compatible and shadow
  providers.
- Encrypted local memory with scoped keys, rotation, hard/soft forget and a
  separate key-escrow path.
- Skill registry with static held-out evaluation, runtime canary and rollback.
- Goals, commitments, attention candidates, delegation profiles, personal
  context graph and an evolution ledger.
- Append-only journals with hash chains, replay, audit receipts and
  cross-process write protection.
- Single runtime home with init/doctor/export/restore/upgrade/uninstall and a
  typed Surface API for local integrations.
- Optional Workshop adapter for real execution; without Workshop, the product is
  analysis/planning only.
- A conversational standalone TUI: answers do not open unsolicited command
  menus; `/` exposes a categorized command surface with recent commands,
  keyboard-isolated panels and confirmation for destructive actions.

## What it is not

- Not a thin chat wrapper around one model provider.
- Not a self-modifying agent: authority, policy, audit and persona cannot be
  changed by a proposal.
- Not a multi-tenant SaaS or an organization-wide identity system yet.
- Not a guaranteed-delivery notification service.
- Not G2: the long-running daemon, Routine scheduler and Watchdog are future
  work.
- Not a production-signed release. The current checkpoint is an R1 release
  candidate; external IdP, third-party audit and the Workshop release gate are
  still open.

## Why it is different

1. **Evidence first**: journals, receipts, replay and audit are product
   primitives, not logging afterthoughts.
2. **Local-first secrets**: keys stay under the runtime home; exports exclude
   key material; escrow is explicit and encrypted.
3. **Deny-by-default execution**: tool calls can be authorized by Authority and
   Policy before they run.
4. **Provider-neutral**: no domain dependency on a model SDK; providers are
   adapters behind a manifest and router.
5. **Verifiable claims**: deterministic tests, held-out evaluation and package
   reproducibility are part of the release gate.

## Status and license

Status: **R1 public pre-release, under active development and not
production-stable.**

Platform: **Linux x86_64 is the only validated and publishable platform.**
Windows builds are internal only, untested and not published; macOS, Android and
iOS are planned.

License: **MIT** (see `LICENSE`). Third-party notices are in
`THIRD_PARTY_NOTICES.md` and `THIRD_PARTY_LICENSES.md`.

Community: **Skynet-Club** (<https://skynet.club>) — testers, reviewers and
builders are welcome while the project iterates in the open.

Security and privacy expectations are in `SECURITY.md` and `PRIVACY.md`.
