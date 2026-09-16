#!/usr/bin/env python3
"""Persistent internal HTTP listener for KubeDiagrams rendering.

Runs as the kr0ki-mcp container's own long-running process (its ENTRYPOINT).
kr0ki-server's POST /render/kubediagram proxies here directly over the pod's
localhost network — this replaces the old stdio-JSON-RPC-over-subprocess hop
that containers/kubediagram-mcp/bridge.py used to provide. Internal-only:
never exposed outside the pod's network namespace.
"""

import json
import os
import subprocess
import tempfile
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer
from pathlib import Path
from urllib.parse import parse_qs, urlparse

MAX_MANIFEST_BYTES = 1_048_576


class Handler(BaseHTTPRequestHandler):
    def log_message(self, format, *args):
        pass  # quiet; matches kr0ki-mcp's existing minimal logging

    def do_GET(self):
        if self.path == "/health":
            self._respond(200, b'{"status":"ok","service":"kubediagram-worker"}', "application/json")
            return
        self._respond(404, b'{"error":"not_found"}', "application/json")

    def do_POST(self):
        parsed = urlparse(self.path)
        if parsed.path != "/render":
            self._respond(404, b'{"error":"not_found"}', "application/json")
            return

        output = parse_qs(parsed.query).get("output", ["svg"])[0]
        if output not in {"svg", "dot_json"}:
            self._respond(400, json.dumps({"error": "invalid output"}).encode(), "application/json")
            return

        length = int(self.headers.get("Content-Length", 0))
        if length == 0:
            self._respond(400, json.dumps({"error": "empty manifest"}).encode(), "application/json")
            return
        if length > MAX_MANIFEST_BYTES:
            self._respond(
                400,
                json.dumps({"error": "manifest exceeds the 1 MiB limit"}).encode(),
                "application/json",
            )
            return
        manifest = self.rfile.read(length)

        with tempfile.TemporaryDirectory() as temporary_directory:
            artifact = Path(temporary_directory) / f"diagram.{output}"
            try:
                process = subprocess.run(
                    ["kube-diagrams", "-", "-f", output, "-o", str(artifact)],
                    input=manifest,
                    stdout=subprocess.PIPE,
                    stderr=subprocess.PIPE,
                    timeout=60,
                    check=False,
                )
                if process.returncode != 0:
                    self._respond(422, process.stderr[:2000], "text/plain")
                    return
                if not artifact.is_file():
                    self._respond(500, b"kube-diagrams completed without an output artifact", "text/plain")
                    return
                content_type = "image/svg+xml" if output == "svg" else "application/json"
                self._respond(200, artifact.read_bytes(), content_type)
            except subprocess.TimeoutExpired:
                self._respond(504, json.dumps({"error": "kube-diagrams timed out"}).encode(), "application/json")

    def _respond(self, status, body, content_type):
        self.send_response(status)
        self.send_header("Content-Type", content_type)
        self.send_header("Content-Length", str(len(body)))
        self.end_headers()
        self.wfile.write(body)


def main():
    port = int(os.environ.get("KR0KI_MCP_WORKER_PORT", "8788"))
    server = ThreadingHTTPServer(("0.0.0.0", port), Handler)
    server.serve_forever()


if __name__ == "__main__":
    main()
