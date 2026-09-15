#!/usr/bin/env python3
"""Minimal stdio MCP bridge for a locally running kr0ki service.

The bridge deliberately exposes the narrow HTTP API kr0ki owns.  It does not
accept an arbitrary endpoint from a caller and it does not proxy any Kroki
grammar that kr0ki does not advertise.
"""

import base64
import json
import os
import subprocess
import sys
import urllib.error
import urllib.request


BASE_URL = os.environ.get("KR0KI_URL", "http://host.containers.internal:8787").rstrip("/")
AUTH_TOKEN = os.environ.get("KR0KI_AUTH_TOKEN")

TOOLS = [
    {
        "name": "render_diagram",
        "description": "Render supported diagram source through the local kr0ki service.",
        "inputSchema": {
            "type": "object",
            "required": ["format", "source"],
            "properties": {
                "format": {"type": "string", "description": "kr0ki diagram format slug."},
                "source": {"type": "string", "description": "UTF-8 diagram source."},
                "output": {
                    "type": "string",
                    "enum": ["svg", "png"],
                    "default": "svg",
                },
            },
        },
    },
    {
        "name": "list_formats",
        "description": "List formats currently supported by the local kr0ki service.",
        "inputSchema": {"type": "object", "properties": {}},
    },
    {
        "name": "render_kubernetes_manifest",
        "description": "Render Kubernetes manifest YAML through the internal KubeDiagrams MCP worker.",
        "inputSchema": {
            "type": "object",
            "required": ["manifest"],
            "properties": {
                "manifest": {"type": "string", "description": "Kubernetes YAML manifest bundle."},
                "output": {"type": "string", "enum": ["svg", "dot_json"], "default": "svg"},
            },
        },
    },
]


def response(request_id, result):
    return {"jsonrpc": "2.0", "id": request_id, "result": result}


def error(request_id, code, message):
    return {"jsonrpc": "2.0", "id": request_id, "error": {"code": code, "message": message}}


def request(url, data=None):
    headers = {"Accept": "application/json"}
    if AUTH_TOKEN:
        headers["Authorization"] = f"Bearer {AUTH_TOKEN}"
    if data is not None:
        headers["Content-Type"] = "text/plain; charset=utf-8"
    req = urllib.request.Request(url, data=data, headers=headers, method="POST" if data is not None else "GET")
    with urllib.request.urlopen(req, timeout=30) as result:
        return result.headers.get_content_type(), result.read()


def tool_error(message):
    return {"content": [{"type": "text", "text": message}], "isError": True}


def call_kubediagram_worker(params):
    """Proxy one allow-listed call to the bundled internal stdio MCP worker."""
    manifest = params.get("manifest")
    output = params.get("output", "svg")
    if not isinstance(manifest, str) or not manifest.strip():
        return tool_error("manifest must be a non-empty string")
    if len(manifest.encode("utf-8")) > 1_048_576:
        return tool_error("manifest exceeds the 1 MiB MCP bridge limit")
    if output not in {"svg", "dot_json"}:
        return tool_error("output must be svg or dot_json")

    messages = [
        {"jsonrpc": "2.0", "id": 1, "method": "initialize", "params": {"protocolVersion": "2024-11-05"}},
        {
            "jsonrpc": "2.0",
            "id": 2,
            "method": "tools/call",
            "params": {
                "name": "render_kubernetes_manifest",
                "arguments": {"manifest": manifest, "output": output},
            },
        },
    ]
    try:
        process = subprocess.run(
            ["python3", "/opt/kubediagram-mcp/bridge.py"],
            input="".join(f"{json.dumps(message)}\n" for message in messages),
            text=True,
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
            timeout=65,
            check=False,
        )
    except (OSError, subprocess.TimeoutExpired) as exc:
        return tool_error(f"internal KubeDiagrams MCP worker unavailable: {exc}")

    if process.returncode != 0:
        return tool_error(f"internal KubeDiagrams MCP worker failed: {process.stderr.strip()}")
    for line in reversed(process.stdout.splitlines()):
        try:
            worker_response = json.loads(line)
        except json.JSONDecodeError:
            continue
        if worker_response.get("id") == 2:
            if "error" in worker_response:
                return tool_error(f"internal KubeDiagrams MCP error: {worker_response['error'].get('message', 'unknown error')}")
            return worker_response.get("result", tool_error("internal KubeDiagrams MCP returned no result"))
    return tool_error("internal KubeDiagrams MCP returned an invalid response")


def call_tool(arguments):
    name = arguments.get("name")
    params = arguments.get("arguments", {})
    if name == "list_formats":
        try:
            _, body = request(f"{BASE_URL}/formats")
            return {"content": [{"type": "text", "text": body.decode("utf-8")}]}
        except (urllib.error.URLError, urllib.error.HTTPError, UnicodeDecodeError) as exc:
            return tool_error(f"kr0ki formats request failed: {exc}")
    if name == "render_kubernetes_manifest":
        return call_kubediagram_worker(params)
    if name != "render_diagram":
        return tool_error(f"unknown tool: {name}")

    diagram_format = params.get("format")
    source = params.get("source")
    output = params.get("output", "svg")
    if not isinstance(diagram_format, str) or not isinstance(source, str):
        return tool_error("format and source must be strings")
    if output not in {"svg", "png"}:
        return tool_error("output must be svg or png")
    if not source.strip():
        return tool_error("source must not be empty")
    if len(source.encode("utf-8")) > 1_048_576:
        return tool_error("source exceeds the 1 MiB MCP bridge limit")

    try:
        content_type, body = request(
            f"{BASE_URL}/render/{diagram_format}?output={output}", source.encode("utf-8")
        )
    except urllib.error.HTTPError as exc:
        return tool_error(f"kr0ki rejected the render request ({exc.code}): {exc.read().decode('utf-8', 'replace')}")
    except urllib.error.URLError as exc:
        return tool_error(f"kr0ki render request failed: {exc}")

    if output == "png":
        return {
            "content": [
                {
                    "type": "image",
                    "data": base64.b64encode(body).decode("ascii"),
                    "mimeType": content_type or "image/png",
                }
            ]
        }
    return {"content": [{"type": "text", "text": body.decode("utf-8", "replace")}]}


def handle(message):
    method = message.get("method")
    request_id = message.get("id")
    if method == "initialize":
        return response(
            request_id,
            {
                "protocolVersion": "2024-11-05",
                "capabilities": {"tools": {}},
                "serverInfo": {"name": "kr0ki-mcp", "version": "0.1.0"},
            },
        )
    if method == "tools/list":
        return response(request_id, {"tools": TOOLS})
    if method == "tools/call":
        return response(request_id, call_tool(message.get("params", {})))
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
    except Exception as exc:  # keep the stdio JSON-RPC transport alive for callers
        print(json.dumps(error(None, -32603, f"internal error: {exc}")), flush=True)
