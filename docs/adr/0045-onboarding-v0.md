# ADR 0045: Onboarding v0 — Non-Interactive Profile Setup

- Status: accepted
- Date: 2026-11-11

## Context

The partner's first five minutes matter: the user should be able to set a voice,
per-domain autonomy and boundaries without editing memory journals by hand.
A full interview and context import is a later product slice; v0 needs a
scriptable contract that writes the same memory records the UserModel reads.

## Decision

1. Add `1agent onboard` with:
   - `--voice operator|advisor|companion`;
   - repeatable `--autonomy DOMAIN=LEVEL`;
   - repeatable `--boundary TEXT`;
   - memory journal/payloads/key paths, `--bot-id` and `--user-id`.
2. Onboarding writes ordinary memory records:
   - `MemoryKind::Preference` with `autonomy.<domain>=<level>`;
   - `MemoryKind::Preference` with `voice_mode=<mode>`;
   - `MemoryKind::Feedback` with `boundary: <text>`.
3. The command builds the `UserModel` after writing and prints it as JSON or
   text, so the user sees exactly what was stored.
4. Invalid levels, voices, empty domains or empty boundaries fail closed before
   writing anything.
5. Each run appends new records; the latest preference wins, and boundaries are
   appended. There is no dedupe or interview logic in v0.
6. This is not the full onboarding: no interactive questions, no document/
   calendar/email import, no first-task activation metric yet.

## Consequences

- A user can set up the partner's relationship settings in one command.
- The same records are visible through `1agent user-model`.
- Re-running onboarding is safe for preferences (newest wins) but can duplicate
  boundaries; a later slice adds dedupe.
- The command does not grant execution authority; autonomy remains a
  preference and the autonomy policy still requires approval where needed.

## Evidence

```bash
cargo test --test onboard_cli
```

The test onboards a bot, reads the same profile back through `user-model`, and
proves invalid autonomy/voice values fail closed.
