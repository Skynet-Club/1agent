# ADR 0075: Digest Attention Summary

- Status: accepted
- Date: 2026-11-12

## Context

`digest` summarized runs, schedules, health and alerts but not the two queues
the partner now maintains: the inbox and team handoffs. A morning digest that
omits pending decisions is incomplete.

## Decision

1. `digest` always reports the number of open or due-snoozed inbox items from
   `--inbox-store`.
2. With `--handoff-store PATH --recipient NAME`, it reports open handoffs
   addressed to that person. `--recipient` without `--handoff-store` fails;
   `--handoff-store` without `--recipient` fails.
3. JSON adds `inbox: {open}`, `handoffs: {open, recipient}` and, with
   `--verification-store PATH`, the all-time local verification summary. Text
   prints the counts and summary in Chinese.
4. `--notify-webhook URL --notify-format generic|slack|discord` posts the same
   text summary through the one-way notifier after building it. The notifier's
   HTTPS/loopback and credential policy applies.
5. The digest is otherwise read-only. It does not sync projections, accept
   handoffs or run a maintenance loop.

## Consequences

- A daily summary now includes the two human queues.
- The recipient filter is by the handoff's `to` field, not an authenticated
  identity.
- Inbox items and handoffs can overlap after projection; the digest reports
  them separately and does not deduplicate.
- No scheduling of the digest itself; a caller/cron must invoke it.

## Evidence

```bash
cargo test --test digest_cli
```

The tests cover counts with one inbox item and one open handoff, the count
dropping after acceptance, and the recipient/store validation.
