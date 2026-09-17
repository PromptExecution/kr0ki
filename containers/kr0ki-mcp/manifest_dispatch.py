"""Manifest-driven HTTP dispatch shared by kr0ki MCP and storyb00k.

The server owns tool shapes in ``GET /mcp/tools``. This module only fetches that
manifest and applies its explicitly declared HTTP bindings; it never carries a
second hand-maintained tool table.
"""

import json
import os
import urllib.parse
import urllib.request

AUTH_TOKEN = os.environ.get("KR0KI_AUTH_TOKEN")
_manifest_cache = None


def http_call(method, url, data=None, headers=None):
    merged_headers = {"Accept": "application/json"}
    if AUTH_TOKEN:
        merged_headers["Authorization"] = f"Bearer {AUTH_TOKEN}"
    if data is not None:
        merged_headers["Content-Type"] = "text/plain; charset=utf-8"
    if headers:
        merged_headers.update(headers)
    request = urllib.request.Request(url, data=data, headers=merged_headers, method=method)
    with urllib.request.urlopen(request, timeout=60) as result:
        return result.headers.get_content_type(), result.read()


def fetch_manifest(base_url):
    global _manifest_cache
    if _manifest_cache is None:
        _, body = http_call("GET", f"{base_url.rstrip('/')}/mcp/tools")
        _manifest_cache = json.loads(body)
    return _manifest_cache


def find_tool(manifest, name):
    return next((tool for tool in manifest if tool["name"] == name), None)


def apply_binding(tool, arguments, base_url):
    """Map a manifest HTTP binding to ``(method, url, raw_body)``."""
    binding = tool["httpBinding"]
    path = binding["pathTemplate"]
    query = {}
    body = None
    for arg in binding["args"]:
        value = arguments.get(arg["name"])
        if value is None:
            continue
        if arg["placement"] == "path":
            path = path.replace("{" + arg["name"] + "}", urllib.parse.quote(str(value), safe=""))
        elif arg["placement"] == "query":
            query[arg["name"]] = str(value)
        elif arg["placement"] == "body":
            body = str(value).encode("utf-8")
    url = f"{base_url.rstrip('/')}{path}"
    if query:
        url += "?" + urllib.parse.urlencode(query)
    return binding["method"], url, body
