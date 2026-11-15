# ADR-0135: Browser sandbox and verifiable action cards

- Status: accepted for 0.98 (decisions 1–7, 10–11 implemented; 8 partial,
  9 deferred)
- Date: 2026-11-14 (accepted 2026-11-15)
- Supersedes: none (extends ADR-0053 CUA provenance, ADR-0089 CUA single-shot plan)
- Related: ADR-0118 (kill switch), ADR-0134 (permission presets),
  `docs/0.98-development-plan.md`, `docs/audit-v0.97.md` (P1 O1 SSRF)

## Context

0.97 has a CUA policy but no executor:

- `CuaAction`, `CuaPlan`, `authorize`, `evaluate`, `check_branch`
  (`src/cua.rs:17-211`); `evaluate` denies high-risk + untrusted args and
  requires approval for untrusted args and irreversible high-risk actions.
- `CuaPlan` has no production call site; `cua-check` exercises only the
  stateless policy (`src/main.rs:909`, `8897-8931`).
- The browser tool is read-only Chrome `--dump-dom`, explicitly not a
  computer-use driver (`src/tools/browser.rs:1-5`, `64-65`, `120-133`).
- No per-task profile, proxy, credential injection or takeover; Chrome inherits
  the environment.
- SSRF is checked only on the initial URL; Chrome re-resolves DNS, follows
  redirects and loads subresources (P1 O1, `docs/audit-v0.97.md:57`).

Public 2026 practice converges on: isolated browser profile, allowlisted egress,
action verification against the user request, and default-deny confirmation for
irreversible actions. The Verifiable Action Card (arXiv 2609.18411) reports
0% attack success across 24 scenarios with a ground-truth action descriptor and
execution binding, at 78% legitimate completion and 0% false-block. Claude in
Chrome's GA defense uses probes plus an action-verification classifier. These
are architectural defenses, not a claim that prompt injection is solved.

## Decisions

1. **Chrome for Testing + per-task profile.** Pin a CfT version; launch with a
   dedicated `--user-data-dir` under
   `$ONEAGENT_HOME/browser/profiles/<task-id>/` (Chrome 136+ requires a custom
   profile for remote debugging). Ephemeral by default; persistent profile is
   explicit opt-in. Never use the user's daily profile, extensions, password
   manager or sync.
2. **CDP driver behind a trait.** Use `chromiumoxide` 0.9 behind
   `BrowserDriver`; use raw CDP for Fetch/Network security domains. Pin the
   protocol version and add a protocol-drift canary test. Keep a raw escape
   hatch so a drift does not block a security fix.
3. **Local CONNECT egress proxy per task.** Allowlist; resolve → validate →
   pin; reject non-global IPs, metadata addresses, CGNAT, IPv4-mapped, NAT64;
   revalidate every redirect; restrict downloads by origin; proxy token is
   loopback-only. CDP `Fetch`/`Network.setBlockedURLs` is defense-in-depth,
   not the primary boundary.
4. **Action classes**: `read`, `navigate`, `input`, `submit`, `download`,
   `upload`, `auth`, `payment`, `delete`, `irreversible`. Irreversible actions
   are **default-deny**.
5. **Verifiable Action Card** for every irreversible (and policy-selected
   high-risk) action. The card contains: action id, origin, element semantic
   target, data diff, consequence, risk, nonce, expiry. It is rendered
   out-of-band in trusted UI; execution re-verifies the exact descriptor and
   nonce. Model-generated confirmation text is never authorization.
   Implemented contract: cards persist at
   `$ONEAGENT_HOME/browser/approvals.json` (0600), bound to the browser task id
   and a SHA-256 of the exact DOM descriptor plus typed-value digest. The tool
   returns `approval_required` with the card id; a human runs
   `1agent browser approvals`, then `1agent browser approve <card-id>` (TTY
   prompt; `--yes` is an explicit declaration) or `deny`. Approval is
   single-use, expires after 120s, and is re-checked inside the card-store
   lock immediately before the CDP action. Buttons, submits, cross-origin or
   download links, password-field secret entry and Enter require a card.
   Reversible same-origin links and plain form focus do not.
6. **Credential broker at the action layer.** Model sees
   `secret://<provider>/<label>`; runtime resolves at the action point;
   origin-bound; one-time consent; no prompt/journal; screenshot masks input
   fields; audit records secret id + origin, never the value. 0.98 does not
   MITM TLS; HTTP-header injection is a 0.98.1 candidate.
7. **Human takeover via a control lease.** Headed Chrome first: agent pauses,
   human operates directly, explicit resume. Headless screencast + local web
   control is a stretch. Takeover events go to the journal.
8. **Recording**: CDP `Page.startScreencast` frames + action JSONL; optional
   ffmpeg WebM. Retention is configurable.
9. **Process sandbox**: keep Chrome's built-in sandbox; **never pass
   `--no-sandbox`**. High-risk tasks may wrap the browser in bwrap + Landlock +
   seccomp; if unprivileged user namespaces are unavailable, degrade and report
   in `doctor`, do not silently disable the sandbox.
10. **Rule of Two**: a browser session may satisfy at most two of
    (untrusted input, sensitive data, state change/external communication).
    If all three are required, require human-in-the-loop or split the session.
11. **No universal injection immunity claim.** Page content is tagged as
    untrusted data, never instructions; actions are verified before execution;
    irreversible actions require confirmation; red-team results are
    suite-specific and reported with ASR, utility, false-block, sample size
    and confidence intervals.

## Consequences

- 0.98 browser support is Linux x86_64 first, matching the release platform.
- A pinned CfT binary adds package size and an update path; `doctor` must
  report the version and a missing binary must produce `next:` guidance.
- The egress proxy is a new security-critical component; it needs its own
  tests for DNS rebinding, redirects, IPv6, metadata and downloads.
- Typing a secret into a page still exposes it to that page's JavaScript. The
  boundary is "trusted origin + explicit consent + no model exposure", not
  "the page cannot read it". This limitation must be documented.
- Headless remote takeover is deferred; headed takeover is the 0.98
  requirement.
- bwrap/Landlock support is best-effort; the plan must not claim OS-level
  isolation when user namespaces are unavailable. Deferred in 0.98.0; Chrome's
  own sandbox remains enabled.
- The approval card is a human-consent and audit control, not a hard boundary
  against a compromised model that also has terminal-write permission. W2 runs
  with the terminal tool unavailable/read-only so the model cannot invoke
  `browser approve` itself.

## Tests

- Per-task profile isolation: two tasks cannot see each other's cookies/storage.
- `--no-sandbox` is never emitted; launch fails closed if Chrome sandbox is
  unavailable and the unsafe flag was requested.
- Egress: private IP, 169.254.169.254, DNS rebinding, redirect-to-private,
  IPv4-mapped, NAT64, download from non-allowlisted origin → blocked.
- Action card: `tests/browser_tool_approval.rs` (live Chrome) proves an
  irreversible button click returns `approval_required`, a fake page
  "Approved" button/cross-origin authorize link/download link cannot
  self-authorize, approval is consumed once, replay fails, and a changed
  descriptor cannot reuse the old approval. `tests/browser_approvals_cli.rs`
  covers list/approve/deny and the no-TTY refusal.
- Credential broker: no secret in tool output/metadata; plaintext in a password
  field is refused; origin mismatch refused; audit contains id + origin only.
- Takeover: pause → human input → resume; agent cannot act during human lease.
- Recording: frames + action JSONL replay to the same action sequence.
