import json
import os
import threading
import unittest
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer

from oneagent_client import Client, ClientError


class Handler(BaseHTTPRequestHandler):
    server_version = "1agent-test"

    def log_message(self, *_args):
        pass

    def _send(self, code, payload):
        raw = payload if isinstance(payload, bytes) else json.dumps(payload).encode()
        self.send_response(code)
        self.send_header("content-type", "application/json")
        self.send_header("content-length", str(len(raw)))
        self.end_headers()
        self.wfile.write(raw)

    def do_GET(self):
        auth = self.headers.get("authorization")
        if self.path == "/v0/health":
            self._send(200, {"ok": True, "version": "test"})
        elif self.path == "/v0/agent-card":
            self._send(200, {"name": "1Agent", "auth": auth})
        elif self.path == "/v0/trust-center":
            self._send(200, {"schema_version": 1})
        elif self.path == "/v0/bad-json":
            self._send(200, b"not-json")
        elif self.path == "/v0/large":
            self._send(200, b"x" * (4 * 1024 * 1024 + 10))
        else:
            self._send(404, {"error": "not found"})

    def do_POST(self):
        length = int(self.headers.get("content-length", "0"))
        body = json.loads(self.rfile.read(length))
        if self.path != "/v0/surface":
            self._send(404, {"error": "not found"})
            return
        self._send(
            200,
            {
                "protocol_version": body.get("protocol_version"),
                "request_id": body.get("request_id"),
                "trace_id": body.get("trace_id"),
                "ok": True,
                "result": {"echo": body.get("method")},
                "auth": self.headers.get("authorization"),
            },
        )


class ClientTest(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.server = ThreadingHTTPServer(("127.0.0.1", 0), Handler)
        cls.thread = threading.Thread(target=cls.server.serve_forever, daemon=True)
        cls.thread.start()
        host, port = cls.server.server_address
        cls.base_url = f"http://{host}:{port}"

    @classmethod
    def tearDownClass(cls):
        cls.server.shutdown()
        cls.server.server_close()
        cls.thread.join(timeout=5)

    def test_read_routes(self):
        client = Client(self.base_url)
        self.assertEqual(client.health()["ok"], True)
        self.assertEqual(client.agent_card()["name"], "1Agent")
        self.assertEqual(client.trust_center()["schema_version"], 1)

    def test_surface_includes_protocol_and_bearer_token(self):
        client = Client(self.base_url, token="secret-token")
        response = client.surface(
            {"name": "thread_list", "params": {"limit": 5}},
            request_id="req-1",
            trace_id="trace-1",
        )
        self.assertEqual(response["protocol_version"], "v0")
        self.assertEqual(response["request_id"], "req-1")
        self.assertEqual(response["trace_id"], "trace-1")
        self.assertEqual(response["result"]["echo"]["name"], "thread_list")
        self.assertEqual(response["auth"], "Bearer secret-token")

    def test_token_is_optional_and_env_backed(self):
        previous = os.environ.pop("ONEAGENT_API_TOKEN", None)
        try:
            client = Client(self.base_url)
            self.assertIsNone(client.agent_card()["auth"])
            os.environ["ONEAGENT_API_TOKEN"] = "env-token"
            self.assertEqual(Client(self.base_url).token, "env-token")
        finally:
            os.environ.pop("ONEAGENT_API_TOKEN", None)
            if previous is not None:
                os.environ["ONEAGENT_API_TOKEN"] = previous

    def test_http_error_and_invalid_json_raise(self):
        client = Client(self.base_url)
        with self.assertRaisesRegex(ClientError, "HTTP 404"):
            client._request("GET", "/v0/missing")
        with self.assertRaisesRegex(ClientError, "invalid JSON"):
            client._request("GET", "/v0/bad-json")

    def test_oversized_response_raises(self):
        client = Client(self.base_url)
        with self.assertRaisesRegex(ClientError, "response too large"):
            client._request("GET", "/v0/large")

    def test_invalid_base_url_is_rejected(self):
        with self.assertRaises(ValueError):
            Client("ftp://example.com")


if __name__ == "__main__":
    unittest.main()
