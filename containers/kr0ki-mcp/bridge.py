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
import re
import sys
import urllib.error
import urllib.parse
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


MCP_HEADERS = {"X-Kr0ki-Transport": "mcp"}


def fetch_list(path):
    """A manifest the server publishes (`/mcp/resources`, `/mcp/prompts`)."""
    _, body = manifest_dispatch.http_call("GET", f"{BASE_URL}{path}", headers=MCP_HEADERS)
    return json.loads(body)


def resource_templates():
    return [
        {
            "uriTemplate": r["uriTemplate"],
            "name": r["name"],
            "description": r["description"],
            "mimeType": r["mimeType"],
        }
        for r in fetch_list("/mcp/resources")
    ]


def match_resource(uri, manifest):
    """The manifest entry whose `uriTemplate` matches `uri`, with its variables decoded."""
    for entry in manifest:
        pattern = re.escape(entry["uriTemplate"])
        pattern = re.sub(r"\\\{(\w+)\\\}", r"(?P<\1>[^/]+)", pattern)
        m = re.fullmatch(pattern, uri)
        if m:
            return entry, {k: urllib.parse.unquote(v) for k, v in m.groupdict().items()}
    return None, None


def read_resource(uri):
    """Resolve a resource URI to the GET route that serves it. No logic of its own: the server
    authorises it exactly as it authorises the same request made directly."""
    entry, variables = match_resource(uri, fetch_list("/mcp/resources"))
    if entry is None:
        raise KeyError(f"unknown resource: {uri}")
    binding = entry["httpBinding"]
    path, query = binding["pathTemplate"], {}
    for arg in binding["args"]:
        value = variables.get(arg["name"])
        if value is None:
            continue
        if arg["placement"] == "path":
            path = path.replace("{" + arg["name"] + "}", urllib.parse.quote(value, safe=""))
        else:
            query[arg["name"]] = value
    url = f"{BASE_URL}{path}" + ("?" + urllib.parse.urlencode(query) if query else "")
    content_type, body = manifest_dispatch.http_call("GET", url, headers=MCP_HEADERS)
    return {"uri": uri, "mimeType": content_type or entry["mimeType"], "text": body.decode("utf-8", "replace")}


def render_prompt(prompt, arguments):
    """Fill `{{arg}}` placeholders once (a value is never re-expanded). A required argument must
    be present and non-empty; an argument the prompt does not declare is refused."""
    declared = {a["name"]: a for a in prompt["arguments"]}
    unknown = [k for k in arguments if k not in declared]
    if unknown:
        raise ValueError(f"unknown argument: {unknown[0]}")
    values = {}
    for name, spec in declared.items():
        value = str(arguments.get(name, "")).strip()
        if not value:
            if spec["required"]:
                raise ValueError(f"prompt {prompt['name']} requires argument {name}")
            continue
        values[name] = value
    return re.sub(r"\{\{(\w+)\}\}", lambda m: values.get(m.group(1), m.group(0)), prompt["template"])


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
        content_type, response_body = manifest_dispatch.http_call(
            method, url, body, headers={"X-Kr0ki-Transport": "mcp"}
        )
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
                "capabilities": {"tools": {}, "resources": {}, "prompts": {}},
                "serverInfo": {"name": "kr0ki-mcp", "version": "0.2.0"},
            },
        )
    if method == "tools/list":
        try:
            return response(request_id, {"tools": mcp_tools_list()})
        except (urllib.error.URLError, urllib.error.HTTPError) as exc:
            return error(request_id, -32000, f"failed to fetch tool manifest: {exc}")
    if method == "resources/templates/list":
        try:
            return response(request_id, {"resourceTemplates": resource_templates()})
        except (urllib.error.URLError, urllib.error.HTTPError) as exc:
            return error(request_id, -32000, f"failed to fetch resources: {exc}")
    if method == "resources/list":
        return response(request_id, {"resources": []})
    if method == "resources/read":
        uri = message.get("params", {}).get("uri", "")
        try:
            return response(request_id, {"contents": [read_resource(uri)]})
        except KeyError as exc:
            return error(request_id, -32602, str(exc.args[0]))
        except urllib.error.HTTPError as exc:
            return error(request_id, -32000, f"kr0ki rejected the request ({exc.code}): {exc.read().decode('utf-8', 'replace')}")
        except urllib.error.URLError as exc:
            return error(request_id, -32000, f"kr0ki request failed: {exc}")
    if method == "prompts/list":
        try:
            return response(
                request_id,
                {"prompts": [{"name": p["name"], "description": p["description"], "arguments": p["arguments"]} for p in fetch_list("/mcp/prompts")]},
            )
        except (urllib.error.URLError, urllib.error.HTTPError) as exc:
            return error(request_id, -32000, f"failed to fetch prompts: {exc}")
    if method == "prompts/get":
        params = message.get("params", {})
        try:
            prompt = next((p for p in fetch_list("/mcp/prompts") if p["name"] == params.get("name")), None)
            if prompt is None:
                return error(request_id, -32602, f"unknown prompt: {params.get('name')}")
            text = render_prompt(prompt, params.get("arguments") or {})
        except ValueError as exc:
            return error(request_id, -32602, str(exc))
        except (urllib.error.URLError, urllib.error.HTTPError) as exc:
            return error(request_id, -32000, f"failed to fetch prompts: {exc}")
        return response(
            request_id,
            {"description": prompt["description"], "messages": [{"role": "user", "content": {"type": "text", "text": text}}]},
        )
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
