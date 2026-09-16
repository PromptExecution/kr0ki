#!/usr/bin/env python3
"""Unit tests for bridge.py's generic manifest-driven dispatch. Run with:
   cd containers/kr0ki-mcp && python3 test_bridge.py
"""
import json
import unittest
from unittest.mock import patch

import bridge

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
        bridge._manifest_cache = None

    @patch("bridge.http_call")
    def test_tools_list_uses_fetched_manifest(self, mock_http_call):
        mock_http_call.return_value = ("application/json", json.dumps(SAMPLE_MANIFEST).encode())
        result = bridge.handle({"jsonrpc": "2.0", "id": 1, "method": "tools/list"})
        names = [t["name"] for t in result["result"]["tools"]]
        self.assertEqual(names, ["render_diagram", "list_formats"])
        mock_http_call.assert_called_once_with("GET", f"{bridge.BASE_URL}/mcp/tools")

    @patch("bridge.http_call")
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

    @patch("bridge.http_call")
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

    @patch("bridge.http_call")
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
        bridge._manifest_cache = SAMPLE_MANIFEST
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


if __name__ == "__main__":
    unittest.main()
