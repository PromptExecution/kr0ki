#!/usr/bin/env python3
"""Unit tests for http_worker.py. Run with:
   cd containers/kr0ki-mcp && python3 test_http_worker.py
"""
import http.client
import json
import subprocess
import threading
import unittest
from unittest.mock import Mock, patch

import http_worker


class HttpWorkerTest(unittest.TestCase):
    def setUp(self):
        self.server = http_worker.ThreadingHTTPServer(("127.0.0.1", 0), http_worker.Handler)
        self.port = self.server.server_address[1]
        self.thread = threading.Thread(target=self.server.serve_forever, daemon=True)
        self.thread.start()

    def tearDown(self):
        self.server.shutdown()
        self.server.server_close()

    def _conn(self):
        return http.client.HTTPConnection("127.0.0.1", self.port, timeout=5)

    def test_health_returns_ok(self):
        conn = self._conn()
        conn.request("GET", "/health")
        resp = conn.getresponse()
        self.assertEqual(resp.status, 200)
        self.assertEqual(json.loads(resp.read())["status"], "ok")

    def test_unknown_path_is_404(self):
        conn = self._conn()
        conn.request("GET", "/nope")
        resp = conn.getresponse()
        self.assertEqual(resp.status, 404)

    def test_render_with_empty_body_is_400(self):
        conn = self._conn()
        conn.request("POST", "/render", body=b"")
        resp = conn.getresponse()
        self.assertEqual(resp.status, 400)
        self.assertIn(b"empty manifest", resp.read())

    def test_render_with_invalid_output_is_400(self):
        conn = self._conn()
        conn.request("POST", "/render?output=png", body=b"apiVersion: v1\nkind: Pod")
        resp = conn.getresponse()
        self.assertEqual(resp.status, 400)

    def test_render_over_size_limit_is_400(self):
        conn = self._conn()
        oversized = b"a" * (http_worker.MAX_MANIFEST_BYTES + 1)
        conn.request("POST", "/render", body=oversized)
        resp = conn.getresponse()
        self.assertEqual(resp.status, 400)
        self.assertIn(b"1 MiB", resp.read())

    @patch("http_worker.subprocess.run")
    def test_render_success_returns_artifact_bytes(self, mock_run):
        def fake_run(cmd, input, stdout, stderr, timeout, check):
            artifact_path = cmd[cmd.index("-o") + 1]
            with open(artifact_path, "wb") as f:
                f.write(b"<svg>fake</svg>")
            return Mock(returncode=0, stderr=b"")

        mock_run.side_effect = fake_run
        conn = self._conn()
        conn.request("POST", "/render", body=b"apiVersion: v1\nkind: Pod")
        resp = conn.getresponse()
        self.assertEqual(resp.status, 200)
        self.assertEqual(resp.read(), b"<svg>fake</svg>")
        self.assertEqual(resp.getheader("Content-Type"), "image/svg+xml")

    @patch("http_worker.subprocess.run")
    def test_render_failure_returns_422(self, mock_run):
        mock_run.return_value = Mock(returncode=1, stderr=b"bad manifest")
        conn = self._conn()
        conn.request("POST", "/render", body=b"not a manifest")
        resp = conn.getresponse()
        self.assertEqual(resp.status, 422)
        self.assertIn(b"bad manifest", resp.read())

    @patch("http_worker.subprocess.run")
    def test_render_timeout_returns_504(self, mock_run):
        mock_run.side_effect = subprocess.TimeoutExpired("kube-diagrams", 60)
        conn = self._conn()
        conn.request("POST", "/render", body=b"apiVersion: v1\nkind: Pod")
        resp = conn.getresponse()
        self.assertEqual(resp.status, 504)
        self.assertIn(b"timed out", resp.read())


if __name__ == "__main__":
    unittest.main()
