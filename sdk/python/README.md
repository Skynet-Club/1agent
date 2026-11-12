# 1Agent Python client (read-only MVP)

Standard-library client for the local 1Agent HTTP surface. It wraps:

- `GET /v0/health`
- `GET /v0/agent-card`
- `GET /v0/trust-center`
- `POST /v0/surface`

It does not start a server, manage credentials, retry writes or hide errors.

```python
from oneagent_client import Client

client = Client("http://127.0.0.1:8787")  # token from ONEAGENT_API_TOKEN
print(client.health())
print(client.surface(
    {"name": "thread_list", "params": {"limit": 10}},
    request_id="req-1",
    trace_id="trace-1",
))
```

Security notes:

- The bearer token comes from `ONEAGENT_API_TOKEN` or the `token` argument.
  Never log the token or raw request headers.
- Responses are capped at 4 MiB; larger responses raise `ClientError`.
- HTTP and JSON failures raise `ClientError`; there is no silent retry.
- The surface `POST` is not idempotent on the client side; pass an
  `idempotency_key` when the server supports it.

Tests:

```bash
cd sdk/python
python3 -m unittest discover -s . -p 'test_*.py'
```
