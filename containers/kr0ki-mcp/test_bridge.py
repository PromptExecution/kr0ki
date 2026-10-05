#!/usr/bin/env python3
"""Unit tests for bridge.py's generic manifest-driven dispatch. Run with:
   cd containers/kr0ki-mcp && python3 test_bridge.py
"""
import json
import unittest
import urllib.error
from unittest.mock import patch

import bridge
import manifest_dispatch

SAMPLE_MANIFEST = [
    {
        "name": "render_diagram",
        "description": "Render supported diagram source through the local kr0ki service.",
        "inputSchema": {"type": "object"},
        "httpBinding": {
            "method": "POST",
            "pathTemplate": "/render/{format}",
            "args": [
                {"name": "format", "placement": "path"},
                {"name": "source", "placement": "body"},
                {"name": "output", "placement": "query"},
            ],
        },
    },
    {
        "name": "list_formats",
        "description": "List formats currently supported by the local kr0ki service.",
        "inputSchema": {"type": "object"},
        "httpBinding": {"method": "GET", "pathTemplate": "/formats", "args": []},
    },
]


class BridgeDispatchTest(unittest.TestCase):
    def setUp(self):
        manifest_dispatch._manifest_cache = None

    @patch("manifest_dispatch.http_call")
    def test_tools_list_uses_fetched_manifest(self, mock_http_call):
        mock_http_call.return_value = ("application/json", json.dumps(SAMPLE_MANIFEST).encode())
        result = bridge.handle({"jsonrpc": "2.0", "id": 1, "method": "tools/list"})
        names = [t["name"] for t in result["result"]["tools"]]
        self.assertEqual(names, ["render_diagram", "list_formats"])
        mock_http_call.assert_called_once_with("GET", f"{bridge.BASE_URL}/mcp/tools")

    @patch("manifest_dispatch.http_call")
    def test_render_diagram_call_maps_path_body_and_query(self, mock_http_call):
        mock_http_call.side_effect = [
            ("application/json", json.dumps(SAMPLE_MANIFEST).encode()),
            ("image/svg+xml", b"<svg>ok</svg>"),
        ]
        result = bridge.handle(
            {
                "jsonrpc": "2.0",
                "id": 2,
                "method": "tools/call",
                "params": {
                    "name": "render_diagram",
                    "arguments": {"format": "d2", "source": "a -> b", "output": "svg"},
                },
            }
        )
        method, url, body = mock_http_call.call_args_list[1][0]
        self.assertEqual(method, "POST")
        self.assertEqual(url, f"{bridge.BASE_URL}/render/d2?output=svg")
        self.assertEqual(body, b"a -> b")
        self.assertEqual(result["result"]["content"][0]["text"], "<svg>ok</svg>")

    @patch("manifest_dispatch.http_call")
    def test_list_formats_call_has_no_body_and_no_query(self, mock_http_call):
        mock_http_call.side_effect = [
            ("application/json", json.dumps(SAMPLE_MANIFEST).encode()),
            ("application/json", b'["d2","plantuml"]'),
        ]
        bridge.handle(
            {
                "jsonrpc": "2.0",
                "id": 3,
                "method": "tools/call",
                "params": {"name": "list_formats", "arguments": {}},
            }
        )
        method, url, body = mock_http_call.call_args_list[1][0]
        self.assertEqual(method, "GET")
        self.assertEqual(url, f"{bridge.BASE_URL}/formats")
        self.assertIsNone(body)

    @patch("manifest_dispatch.http_call")
    def test_unknown_tool_name_is_a_tool_error(self, mock_http_call):
        mock_http_call.return_value = ("application/json", json.dumps(SAMPLE_MANIFEST).encode())
        result = bridge.handle(
            {
                "jsonrpc": "2.0",
                "id": 4,
                "method": "tools/call",
                "params": {"name": "does_not_exist", "arguments": {}},
            }
        )
        self.assertTrue(result["result"]["isError"])
        self.assertIn("unknown tool", result["result"]["content"][0]["text"])

    def test_oversized_source_is_rejected_before_any_http_call(self):
        manifest_dispatch._manifest_cache = SAMPLE_MANIFEST
        result = bridge.handle(
            {
                "jsonrpc": "2.0",
                "id": 5,
                "method": "tools/call",
                "params": {
                    "name": "render_diagram",
                    "arguments": {"format": "d2", "source": "a" * 1_048_577},
                },
            }
        )
        self.assertTrue(result["result"]["isError"])
        self.assertIn("1 MiB", result["result"]["content"][0]["text"])

    @patch("manifest_dispatch.http_call")
    def test_manifest_fetch_failure_during_tools_call_preserves_request_id(self, mock_http_call):
        manifest_dispatch._manifest_cache = None
        mock_http_call.side_effect = urllib.error.URLError("connection refused")
        result = bridge.handle(
            {
                "jsonrpc": "2.0",
                "id": 42,
                "method": "tools/call",
                "params": {"name": "render_diagram", "arguments": {}},
            }
        )
        self.assertEqual(result["id"], 42)
        self.assertNotIn("error", result)
        self.assertTrue(result["result"]["isError"])

    @patch("manifest_dispatch.urllib.request.urlopen")
    def test_http_call_headers_override_defaults(self, urlopen):
        response = urlopen.return_value.__enter__.return_value
        response.headers.get_content_type.return_value = "application/json"
        response.read.return_value = b"{}"
        manifest_dispatch.http_call(
            "POST", "http://example.invalid", b"x",
            headers={"Authorization": "Bearer custom", "Content-Type": "application/json"},
        )
        request = urlopen.call_args.args[0]
        self.assertEqual(request.get_header("Authorization"), "Bearer custom")
        self.assertEqual(request.get_header("Content-type"), "application/json")



RESOURCES = [
    {
        "uriTemplate": "kr0ki://requirement/{id}",
        "name": "requirement",
        "description": "d",
        "mimeType": "application/json",
        "httpBinding": {"method": "GET", "pathTemplate": "/assurance/requirements/{id}", "args": [{"name": "id", "placement": "path"}]},
    },
    {
        "uriTemplate": "kr0ki://evidence/{requirement}",
        "name": "evidence",
        "description": "d",
        "mimeType": "application/json",
        "httpBinding": {"method": "GET", "pathTemplate": "/assurance/evidence", "args": [{"name": "requirement", "placement": "query"}]},
    },
]

PROMPTS = [
    {
        "name": "explain_gap",
        "description": "Explain a gap.",
        "arguments": [{"name": "id", "description": "Requirement id.", "required": True}],
        "template": "Explain {{id}}: call trace_requirement with id={{id}}.",
    },
    {"name": "next", "description": "d", "arguments": [], "template": "Do the next slice."},
]


class ResourcesAndPromptsTest(unittest.TestCase):
    def test_a_resource_uri_resolves_to_its_get_route_with_path_and_query_args(self):
        with patch("manifest_dispatch.http_call") as call:
            call.side_effect = [
                ("application/json", json.dumps(RESOURCES).encode()),
                ("application/json", b'{"requirement": {"id": "KR-A01"}}'),
            ]
            result = bridge.handle({"jsonrpc": "2.0", "id": 1, "method": "resources/read", "params": {"uri": "kr0ki://requirement/KR-A01"}})
            self.assertEqual(call.call_args_list[1][0][:2], ("GET", f"{bridge.BASE_URL}/assurance/requirements/KR-A01"))
            self.assertEqual(call.call_args_list[1][1]["headers"], {"X-Kr0ki-Transport": "mcp"})
            self.assertIn("KR-A01", result["result"]["contents"][0]["text"])

            call.side_effect = [("application/json", json.dumps(RESOURCES).encode()), ("application/json", b"{}")]
            bridge.handle({"jsonrpc": "2.0", "id": 2, "method": "resources/read", "params": {"uri": "kr0ki://evidence/KR-A02"}})
            self.assertEqual(call.call_args_list[3][0][1], f"{bridge.BASE_URL}/assurance/evidence?requirement=KR-A02")

    def test_a_path_variable_is_percent_encoded_so_it_cannot_change_the_route(self):
        with patch("manifest_dispatch.http_call") as call:
            call.side_effect = [("application/json", json.dumps(RESOURCES).encode()), ("application/json", b"{}")]
            bridge.handle({"jsonrpc": "2.0", "id": 1, "method": "resources/read", "params": {"uri": "kr0ki://requirement/..%2Fadmin"}})
            self.assertNotIn("/../admin", call.call_args_list[1][0][1])
            self.assertTrue(call.call_args_list[1][0][1].endswith("..%2Fadmin"))

    def test_an_unknown_resource_is_an_error_not_a_guess(self):
        with patch("manifest_dispatch.http_call") as call:
            call.return_value = ("application/json", json.dumps(RESOURCES).encode())
            result = bridge.handle({"jsonrpc": "2.0", "id": 1, "method": "resources/read", "params": {"uri": "kr0ki://nope/x"}})
            self.assertEqual(result["error"]["code"], -32602)

    def test_a_server_refusal_on_a_resource_read_is_surfaced_with_its_status(self):
        err = urllib.error.HTTPError("u", 403, "Forbidden", {}, __import__("io").BytesIO(b'{"error":"forbidden"}'))
        with patch("manifest_dispatch.http_call") as call:
            call.side_effect = [("application/json", json.dumps(RESOURCES).encode()), err]
            result = bridge.handle({"jsonrpc": "2.0", "id": 1, "method": "resources/read", "params": {"uri": "kr0ki://requirement/KR-A01"}})
            self.assertIn("403", result["error"]["message"])

    def test_prompts_list_and_get_render_arguments_once(self):
        with patch("manifest_dispatch.http_call") as call:
            call.return_value = ("application/json", json.dumps(PROMPTS).encode())
            listed = bridge.handle({"jsonrpc": "2.0", "id": 1, "method": "prompts/list"})
            self.assertEqual([p["name"] for p in listed["result"]["prompts"]], ["explain_gap", "next"])
            got = bridge.handle({"jsonrpc": "2.0", "id": 2, "method": "prompts/get", "params": {"name": "explain_gap", "arguments": {"id": "KR-A06"}}})
            text = got["result"]["messages"][0]["content"]["text"]
            self.assertIn("trace_requirement with id=KR-A06", text)
            self.assertNotIn("{{", text)

    def test_prompt_arguments_are_checked_and_never_re_expanded(self):
        with patch("manifest_dispatch.http_call") as call:
            call.return_value = ("application/json", json.dumps(PROMPTS).encode())
            def get(args):
                return bridge.handle({"jsonrpc": "2.0", "id": 1, "method": "prompts/get", "params": {"name": "explain_gap", "arguments": args}})
            self.assertEqual(get({})["error"]["code"], -32602)
            self.assertEqual(get({"id": "  "})["error"]["code"], -32602)
            self.assertEqual(get({"id": "x", "evil": "y"})["error"]["code"], -32602)
            injected = get({"id": "{{id}}"})["result"]["messages"][0]["content"]["text"]
            self.assertIn("id={{id}}", injected)

    def test_initialize_advertises_tools_resources_and_prompts(self):
        caps = bridge.handle({"jsonrpc": "2.0", "id": 1, "method": "initialize"})["result"]["capabilities"]
        self.assertEqual(set(caps), {"tools", "resources", "prompts"})


if __name__ == "__main__":
    unittest.main()
