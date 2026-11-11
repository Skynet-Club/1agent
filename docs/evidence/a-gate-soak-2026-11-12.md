# A-Gate Soak — Evidence (2026-11-12)

Status: **5/5 cycles passed on this machine**  
Scope: the local software soak test, not the external A-Gate/W-Gate checklist.

## Command

```bash
for i in 1 2 3 4 5; do
  cargo test --offline --test a_gate_soak -- --ignored
done
```

## Expected results

```text
test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.21s
test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.21s
test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.21s
test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.21s
test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.21s
```

Each cycle ingests 300 task-graph triggers/evals, 200 memory records and skill
records, then checks audit and replay consistency.

## What this proves

- Repeated local soak cycles complete without failure.
- Task-graph, memory and skill journals remain auditable/replayable in the
  exercised volume.

## What this does not prove

- The full 24h multi-agent acceptance run.
- Real provider/network behavior.
- The external A-Gate/W-Gate checklist, external audit or release package.
- Cross-platform runtime behavior; cycles ran on Linux only.

## Files

- `tests/a_gate_soak.rs`
- `docs/claims.md`, `CHANGELOG.md`
