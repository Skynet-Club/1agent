# 1ASH memory family (synthetic fixture)

Five tasks exercise the harness shape for memory: a task seeds `MEMORY.md`, the
subject reads it, writes `answer.txt`, and the scorer checks the answer.

| Task | Fixture pattern |
| --- | --- |
| `memory-01` | recall a stored preference |
| `memory-02` | use the latest updated value |
| `memory-03` | use the most recent dated value |
| `memory-04` | ignore a stale preference |
| `memory-05` | continue a multi-session project decision |

Run:

```sh
python3 harness/1ash/memory/run_memory.py
```

The reference subject is a fixture oracle and the no-op subject must score 0.
This family validates seeding, instruction delivery and scoring. It does **not**
call 1Agent `MemoryService`; LongMemEval, PersonaMem and MemoryArena are the
external benchmarks for retrieval and long-horizon behavior.
