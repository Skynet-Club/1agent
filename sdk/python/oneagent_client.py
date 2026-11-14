"""Minimal stdlib client for the 1Agent local HTTP surface.

The client only wraps the documented read-only routes and the surface POST.
It does not start a server, manage credentials, or retry writes.
"""

from __future__ import annotations

import json
import os
import urllib.error
import urllib.request
from typing import Any

MAX_RESPONSE_BYTES = 4 * 1024 * 1024
DEFAULT_BASE_URL = "http://127.0.0.1:8787"


class ClientError(Exception):
    """A transport, HTTP or JSON failure from the 1Agent surface."""


class Client:
    """Client for `GET /v0/health`, `/v0/agent-card`, `/v0/trust-center`
    and `POST /v0/surface`.

    The bearer token is read from the `token` argument or the
    `ONEAGENT_API_TOKEN` environment variable. Never log the token or the
    request headers.
    """

    def __init__(
        self,
        base_url: str = DEFAULT_BASE_URL,
        token: str | None = None,
        timeout: float = 10.0,
    ) -> None:
        if not base_url.startswith(("http://", "https://")):
            raise ValueError("base_url must start with http:// or https://")
        self.base_url = base_url.rstrip("/")
        self.token = token if token is not None else os.environ.get("ONEAGENT_API_TOKEN")
        self.timeout = timeout

    def health(self) -> dict[str, Any]:
        return self._request("GET", "/v0/health")

    def agent_card(self) -> dict[str, Any]:
        return self._request("GET", "/v0/agent-card")

    def trust_center(self) -> dict[str, Any]:
        return self._request("GET", "/v0/trust-center")

    def surface(
        self,
        method: dict[str, Any],
        *,
        request_id: str,
        trace_id: str,
        protocol_version: str = "v0",
        idempotency_key: str | None = None,
    ) -> dict[str, Any]:
        body: dict[str, Any] = {
            "protocol_version": protocol_version,
            "request_id": request_id,
            "trace_id": trace_id,
            "method": method,
        }
        if idempotency_key is not None:
            body["idempotency_key"] = idempotency_key
        return self._request("POST", "/v0/surface", body)

    def _request(
        self, http_method: str, path: str, body: dict[str, Any] | None = None
    ) -> dict[str, Any]:
        data = json.dumps(body).encode("utf-8") if body is not None else None
        request = urllib.request.Request(
            self.base_url + path, data=data, method=http_method
        )
        request.add_header("content-type", "application/json")
        if self.token:
            request.add_header("authorization", f"Bearer {self.token}")
        try:
            with urllib.request.urlopen(request, timeout=self.timeout) as response:
                raw = response.read(MAX_RESPONSE_BYTES + 1)
        except urllib.error.HTTPError as error:
            try:
                raw = error.read(MAX_RESPONSE_BYTES + 1)
            finally:
                error.close()
            if len(raw) > MAX_RESPONSE_BYTES:
                raise ClientError("response too large") from error
            raise ClientError(
                f"HTTP {error.code}: {raw.decode('utf-8', 'replace')[:512]}"
            ) from error
        except urllib.error.URLError as error:
            raise ClientError(f"request failed: {error.reason}") from error
        if len(raw) > MAX_RESPONSE_BYTES:
            raise ClientError("response too large")
        try:
            return json.loads(raw)
        except json.JSONDecodeError as error:
            raise ClientError("invalid JSON response") from error
