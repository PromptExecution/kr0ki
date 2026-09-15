#!/usr/bin/env python3
"""Stdio MCP bridge for the pinned KubeDiagrams container command.

Only manifest text and a safe output kind are accepted.  KubeDiagrams' -c
option is intentionally absent because its configuration permits exec().
"""

import json
import subprocess
import sys
import tempfile
from pathlib import Path


TOOLS = [{
    "name": "render_kubernetes_manifest",
    "description": "Render Kubernetes manifest YAML through KubeDiagrams without custom configuration.",
    "inputSchema": {
        "type": "object",
        "required": ["manifest"],
        "properties": {
            "manifest": {"type": "string", "description": "Kubernetes YAML manifest bundle."},
            "output": {"type": "string", "enum": ["svg", "dot_json"], "default": "svg"},
        },
    },
}]


def response(request_id, result):
    return {"jsonrpc": "2.0", "id": request_id, "result": result}


def error(request_id, code, message):
    return {"jsonrpc": "2.0", "id": request_id, "error": {"code": code, "message": message}}


def tool_error(message):
    return {"content": [{"type": "text", "text": message}], "isError": True}


def render(params):
    manifest = params.get("manifest")
    output = params.get("output", "svg")
    if not isinstance(manifest, str) or not manifest.strip():
        return tool_error("manifest must be a non-empty string")
    if len(manifest.encode("utf-8")) > 1_048_576:
        return tool_error("manifest exceeds the 1 MiB MCP bridge limit")
    if output not in {"svg", "dot_json"}:
        return tool_error("output must be svg or dot_json")

    with tempfile.TemporaryDirectory() as temporary_directory:
        artifact = Path(temporary_directory) / f"diagram.{output}"
        process = subprocess.run(
            ["kube-diagrams", "-", "-f", output, "-o", str(artifact)],
            input=manifest.encode("utf-8"),
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
            timeout=60,
            check=False,
        )
        if process.returncode != 0:
            return tool_error(
                f"kube-diagrams failed ({process.returncode}): {process.stderr.decode('utf-8', 'replace')}"
            )
        if not artifact.is_file():
            return tool_error("kube-diagrams completed without an output artifact")
        return {"content": [{"type": "text", "text": artifact.read_text(encoding="utf-8")}]}


def handle(message):
    method = message.get("method")
    request_id = message.get("id")
    if method == "initialize":
        return response(request_id, {
            "protocolVersion": "2024-11-05",
            "capabilities": {"tools": {}},
            "serverInfo": {"name": "kubediagram-mcp", "version": "0.1.0"},
        })
    if method == "tools/list":
        return response(request_id, {"tools": TOOLS})
    if method == "tools/call":
        params = message.get("params", {})
        if params.get("name") != "render_kubernetes_manifest":
            return response(request_id, tool_error(f"unknown tool: {params.get('name')}"))
        return response(request_id, render(params.get("arguments", {})))
    if method == "ping":
        return response(request_id, {})
    if request_id is None:
        return None
    return error(request_id, -32601, f"method not found: {method}")


for line in sys.stdin:
    try:
        result = handle(json.loads(line))
        if result is not None:
            print(json.dumps(result), flush=True)
    except json.JSONDecodeError:
        print(json.dumps(error(None, -32700, "parse error")), flush=True)
    except Exception as exc:
        print(json.dumps(error(None, -32603, f"internal error: {exc}")), flush=True)
