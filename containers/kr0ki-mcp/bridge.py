#!/usr/bin/env python3
"""Minimal stdio MCP bridge for a locally running kr0ki service.

Tool definitions (name, schema, HTTP binding) are fetched once from
kr0ki-server's GET /mcp/tools manifest and used to dispatch every
tools/call generically — adding a new McpTool variant on the Rust side
(crates/kr0ki-core/src/mcp_tool.rs) needs zero changes here.
"""

import base64
import json
import os
import sys
import urllib.error
import urllib.request

import manifest_dispatch

BASE_URL = os.environ.get("KR0KI_URL", "http://host.containers.internal:8787").rstrip("/")
AUTH_TOKEN = os.environ.get("KR0KI_AUTH_TOKEN")
MAX_INPUT_BYTES = 1_048_576

def response(request_id, result):
    return {"jsonrpc": "2.0", "id": request_id, "result": result}


def error(request_id, code, message):
    return {"jsonrpc": "2.0", "id": request_id, "error": {"code": code, "message": message}}


def tool_error(message):
    return {"content": [{"type": "text", "text": message}], "isError": True}


def mcp_tools_list():
    return [
        {"name": t["name"], "description": t["description"], "inputSchema": t["inputSchema"]}
        for t in manifest_dispatch.fetch_manifest(BASE_URL)
    ]


def call_tool(arguments):
    name = arguments.get("name")
    params = arguments.get("arguments", {})
    if not isinstance(params, dict):
        return tool_error("arguments must be an object")

    try:
        tool = manifest_dispatch.find_tool(manifest_dispatch.fetch_manifest(BASE_URL), name)
    except (urllib.error.HTTPError, urllib.error.URLError) as exc:
        return tool_error(f"failed to fetch tool manifest: {exc}")

    if tool is None:
        return tool_error(f"unknown tool: {name}")

    for argument in tool["httpBinding"]["args"]:
        if argument["placement"] != "body":
            continue
        value = params.get(argument["name"])
        if isinstance(value, str) and len(value.encode("utf-8")) > MAX_INPUT_BYTES:
            return tool_error("input exceeds the 1 MiB MCP bridge limit")

    method, url, body = manifest_dispatch.apply_binding(tool, params, BASE_URL)
    output = params.get("output", "svg")

    try:
        content_type, response_body = manifest_dispatch.http_call(method, url, body)
    except urllib.error.HTTPError as exc:
        return tool_error(f"kr0ki rejected the request ({exc.code}): {exc.read().decode('utf-8', 'replace')}")
    except urllib.error.URLError as exc:
        return tool_error(f"kr0ki request failed: {exc}")

    if output == "png":
        return {
            "content": [
                {
                    "type": "image",
                    "data": base64.b64encode(response_body).decode("ascii"),
                    "mimeType": content_type or "image/png",
                }
            ]
        }
    return {"content": [{"type": "text", "text": response_body.decode("utf-8", "replace")}]}


def handle(message):
    method = message.get("method")
    request_id = message.get("id")
    if method == "initialize":
        return response(
            request_id,
            {
                "protocolVersion": "2024-11-05",
                "capabilities": {"tools": {}},
                "serverInfo": {"name": "kr0ki-mcp", "version": "0.2.0"},
            },
        )
    if method == "tools/list":
        try:
            return response(request_id, {"tools": mcp_tools_list()})
        except (urllib.error.URLError, urllib.error.HTTPError) as exc:
            return error(request_id, -32000, f"failed to fetch tool manifest: {exc}")
    if method == "tools/call":
        return response(request_id, call_tool(message.get("params", {})))
    if method == "ping":
        return response(request_id, {})
    if request_id is None:
        return None
    return error(request_id, -32601, f"method not found: {method}")


def main():
    for line in sys.stdin:
        try:
            result = handle(json.loads(line))
            if result is not None:
                print(json.dumps(result), flush=True)
        except json.JSONDecodeError:
            print(json.dumps(error(None, -32700, "parse error")), flush=True)
        except Exception as exc:  # keep the stdio JSON-RPC transport alive for callers
            print(json.dumps(error(None, -32603, f"internal error: {exc}")), flush=True)


if __name__ == "__main__":
    main()
