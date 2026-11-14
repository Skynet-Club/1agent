# ADR 0084: Skill Capture from a Completed Journal

- Status: accepted
- Date: 2026-11-12

## Context

The skill lifecycle already supports `SKILL.md` scanning, immutable versions,
held-out evaluation, candidate activation, runtime canary and rollback.
Demonstration capture stores only hashes and lengths. There was no primitive
that turned a finished run into a reviewable `SKILL.md` draft, so operators
had to write skills by hand and could accidentally paste secrets or
machine-specific paths into them.

## Decision

1. Add `skill_capture::capture_journal(path, name, version, description,
   scope)`. It accepts only a regular file, reads at most 64 MiB at read time
   (not only from metadata), selects the last run segment (`run_started` or
   `resume_started`), and requires a `run_stopped` with `Completed`. A
   cancelled, failed or truncated run is rejected. More than 100 successful
   steps fail closed rather than producing an unusable draft.
2. Only tool calls with a matching `tool_observed` status `Ok` for the same
   call id and tool name become procedure steps, in call order. Duplicate call
   ids are captured once. The observation body and model answer are never
   included; the journal remains the evidence, not the skill.
3. Redact secret-like values before writing:
   - prefixed tokens (`sk-`, `ghp_`/`gho_`/`ghu_`/`ghs_`/`ghr_`,
     `github_pat_`, `xoxb-`/`xoxa-`/`xoxp-`/`xoxr-`/`xoxs-`, `AKIA`/`ASIA`);
   - `Bearer` tokens;
   - `key=value` and `key: value` assignments whose key contains
     token/secret/password/api key/authorization/credential;
   - URL userinfo, using the last `@` before the path
     (`https://user:pass@host`).
   JSON arguments are walked recursively; values under secret-named keys are
   replaced with `<redacted>` wholesale, including objects, arrays and nested
   environment maps. Quoted spans inside text are escape-aware (`\"` does not
   close the value). Hyphenated header names are normalized before matching.
   Object keys are themselves sanitized and generalized, including
   secret-named keys and display characters such as `<`/`>`/backticks;
   collisions after sanitization get a `#N` suffix from a per-base counter, so
   many colliding keys stay linear rather than quadratic.
4. Generalize machine-specific values:
   - `/home/<user>`, `/Users/<user>` and `C:\Users\<user>` become `$HOME`;
   - `/tmp/` and `/var/tmp/` become `$TMPDIR/`;
   - UUIDs become `<id>`;
   - email addresses become `<email>`, preserving trailing punctuation;
   - control characters and backticks are replaced with spaces/apostrophes,
     and each step is truncated to 240 characters.
5. Generate a `SKILL.md` draft with `name`, `description`, `version`,
   `scope`, `requires_tools` (unique captured tool names) and
   `risk: medium`. Platform availability is left unspecified. The body has
   When-to-use, Procedure, Verification and Provenance sections. Name,
   version, description and scope must be single-line; tool names with control
   characters fail closed. `SkillManifest::from_markdown` rejects duplicate
   frontmatter keys, and `SkillManifest::validate` rejects unsafe scopes and
   platform/tool entries. The draft is passed through `scan_markdown` before it
   is returned.
6. Add `1agent skill capture --journal PATH --name NAME --version VERSION
   --description TEXT [--scope SCOPE] --out PATH [--force] [--json]`. It
   writes a new file and refuses to overwrite unless `--force` is given. It
   does not register, evaluate or activate the skill.

## Consequences

- Capture is a drafting aid, not a trust decision. A human must review the
  redacted steps, run held-out evaluation and activate the version through the
  existing candidate or registry path.
- Redaction is heuristic. It covers common token shapes and secret-named JSON
  keys, but it is not a full DLP system; a novel secret format can still pass
  through.
- Only successful, completed runs are captured. This keeps failed or partial
  procedures out of the draft, but it also cannot capture a useful recovery
  that ended in `NeedsEvidence`.
- The draft contains no observation bodies, so it describes the procedure but
  not the evidence produced by the run.
- The generated body has no platform list and a fixed `medium` risk; an
  operator may need to edit both before activation.

## Evidence

```bash
cargo test skill_capture --lib
cargo test --test skill_capture_cli
cargo test --test skill_scan_cli
```

The tests cover completed-run capture, rejection of cancelled runs and runs
without successful tools, the 64 MiB limit, secret/assignment/URL redaction,
home/temp/UUID/email generalization, scan compatibility and overwrite refusal.
