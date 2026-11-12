# ADR 0085: Skill Evaluation CLI Gate

- Status: accepted
- Date: 2026-11-12

## Context

The skill library could register, evaluate and activate versions through the
agent tools and through `SkillService`, and `skill capture` now produces a
reviewable `SKILL.md` draft. The CLI had no way to register that draft, run the
static held-out suite, or activate only when the suite passed. An operator had
to go through the agent tool path or build a signed bundle first.

## Decision

1. Add `1agent skill register --store PATH --path SKILL.md [--json]`. It reads
   and scans the file with `scan_markdown`, then registers the immutable
   `name@version`. Duplicate versions and unsafe manifests fail closed.
2. Add `1agent skill eval --store PATH --name NAME --version VERSION --suite
   PATH [--json]`. The suite file is the `SkillEvalSuite` JSON shape:
   `suite_id`, `min_score_bps` and one or more cases with required/forbidden
   markers. Evaluation is static and deterministic; it never runs the skill.
3. Add `1agent skill activate-if-passed --store PATH --name NAME --version
   VERSION --suite PATH [--json]`. It evaluates first and activates only when
   `score_bps >= min_score_bps`; a failing report returns an error and leaves
   the active version unchanged.
4. An invalid suite (unsafe id, empty cases, threshold outside 0..=10000,
   empty markers) fails before any evaluation. A missing version fails closed.
5. This is the static held-out gate, not a runtime canary. Canary promotion
   and rollback remain `SkillCanary`; blind quality evaluation remains future
   work.

## Consequences

- A captured draft can move through register → eval → activate entirely from
  the CLI, with the evaluation report printed as JSON for evidence.
- The operator supplies the suite; a weak suite is not a quality guarantee.
  Marker matching can be gamed, so the suite must be held out from the skill
  author.
- Registration is immutable: changing a skill requires a new version.
- The command does not prove that the skill is safe to execute; execution still
  goes through the existing tool permission and Authority/Policy gates.

## Evidence

```bash
cargo test --test skill_register_eval_cli
cargo test skill --lib
```

The CLI tests cover register → eval → activate, duplicate registration, a
failing evaluation that reports forbidden hits and appends no activation event,
and an invalid suite that fails closed.
