# Python Read-Only SDK — Evidence (2026-11-12)

Status: **passed on this machine**
Scope: `sdk/python/oneagent_client.py` and its fake-server tests.

## Command

```bash
cd sdk/python
python3 -m unittest discover -s . -p 'test_*.py'
```

## Result

```text
......
----------------------------------------------------------------------
Ran 6 tests in 0.524s

OK
```

## What this proves

- Health, agent-card and trust-center routes parse JSON.
- The surface POST sends `v0`, request/trace ids, an echoable method and the
  bearer token when configured.
- The token can come from the constructor or `ONEAGENT_API_TOKEN`; with no
  token no Authorization header is sent.
- HTTP 404, invalid JSON and a response over 4 MiB raise `ClientError`.
- A non-http(s) base URL is rejected.

## What this does not prove

- A real server or real task execution; the tests use a local fake server.
- Task running, streaming, retries, async or typed models; none exist.
- That the future SDK packaging (PyPI, versioning, typing) is ready.
- That the Python tests run in `scripts/verify.sh`; they are manual.
