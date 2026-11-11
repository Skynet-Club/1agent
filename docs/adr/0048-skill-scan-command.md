# ADR 0048: Skill Scan Command and Injection Markers

- Status: accepted
- Date: 2026-11-11

## Context

Skills are distributed as `SKILL.md` files. The registry already validates the
manifest and scans for control characters, private-key markers, secret-like
tokens and a few instruction-override phrases. A user receiving a skill from
outside the registry had no way to run that scan without registering it.

## Decision

1. Add `1agent skill scan --path SKILL.md [--json]`.
2. The command calls `scan_markdown`, which parses with
   `SkillManifest::from_markdown`, validates frontmatter and runs the existing
   `scan_manifest` checks.
3. Add four instruction-override markers to the scan:
   `disregard previous instructions`, `reveal your system prompt`,
   `print your instructions`, `jailbreak`.
4. On success, print name, version and content SHA-256 as JSON or text. On
   rejection, print `{ok:false, error}` as JSON (or a stderr message) and exit
   non-zero.
5. The command does not register, import, sign or execute the skill. It is a
   preflight check, not a malware sandbox.

## Consequences

- A user can reject a suspicious skill before it enters the registry.
- The same scan runs on registration, so the CLI and registry cannot drift on
  these markers.
- Marker matching is heuristic; a clean scan is not a safety proof. Signature
  and permission checks remain separate.

## Evidence

```bash
cargo test skill --lib
cargo test --test skill_scan_cli
```

The tests cover a safe skill, a prompt-injection skill and malformed
frontmatter.
