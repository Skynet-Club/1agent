# Status v0.1.0

Date: 2026-11-01
Status: executable vertical slice

## Verified

| Area | Evidence |
|---|---|
| Build | `cargo build --offline --release` passed |
| Formatting | `cargo fmt --check` passed |
| Lint | `cargo clippy --offline --all-targets -- -D warnings` passed |
| Unit tests | 13 passed |
| CLI end-to-end | Real process run, audit and replay passed |
| Provider adapter | Loopback OpenAI-compatible fixture passed outside the managed sandbox |
| Release binary | 7.0 MiB Linux x86-64 ELF, SHA-256 `4d4d0b059e1871b4da399ded5ab24322da64ec9b28cab04e49c2df6ee65d9fbd` |

## Runtime smoke evidence

```text
run_id: 2ae34bff-db90-4e4e-97fb-e759434dd1be
events: 10
stop_reason: completed
unresolved_tool_calls: []
audit_issues: []
```

The smoke task read `docs/architecture.md` through the read-only tool, produced an evidence
record, stopped with `completed`, replayed successfully and passed structural audit.

## Requirement comparison

| Requirement | Status | Notes |
|---|---|---|
| Result-first structured output | implemented | UTF-8 CLI and JSON output |
| Facts/assumptions/constraints/unknowns/flip conditions | implemented | Structured answer contract |
| Provider-neutral | partial | Deterministic and OpenAI-compatible adapters implemented |
| Provider routing | partial | Static capability filtering implemented; fallback and learned routing pending |
| Read-only tools | implemented | Read-root confined UTF-8 file reader |
| Replayable journal | implemented | Versioned JSONL, replay command |
| Audit | implemented | Sequence, run id, terminal reason and tool pairing checks |
| Read-only default | implemented | Authority false by default; execute mode requires authorization |
| Cross-platform core | partial | No platform branch in `src/`; only Linux x86-64 verified |
| Experience evolution | not started | L2 candidate workflow is the next major slice |
| Real model quality | not verified | No provider credentials or gold reasoning suite in current environment |
| Context compaction | not started | Required before long-session production use |
| Terminal integration | intentionally deferred | Terminal remains frozen at v0.10.21 |

## Known risks

- The deterministic provider validates plumbing only; it is not a reasoning model.
- The OpenAI-compatible adapter is verified against a local fixture, not a live external
  provider.
- Overall run timeout is enforced around provider calls and checked before tools; process-level
  tool cancellation needs a second adapter to verify.
- The JSONL store has no multi-writer lock, encryption or indexing.
- The `read_file` tool does not yet detect invisible Unicode instructions or sanitize file
  content beyond treating it as data in the model contract.
- Linux is the only verified platform; compile portability is not platform support.

## Next requirement

Implement the L2 experience-evolution slice:

1. Candidate memory, skill and route-policy records.
2. Evidence/source and applicability metadata.
3. Approval state machine.
4. Versioning, activation and rollback.
5. Held-out evaluation that rejects harmful or non-generalizable lessons.
