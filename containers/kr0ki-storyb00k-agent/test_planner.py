"""Planner mode: the gallery's planning agent may steer *its own* browser tab and nothing else."""

import json
import tempfile
import threading
import unittest
import urllib.request
from pathlib import Path
from unittest.mock import patch

import server


def tool(name, props=None, required=None, binding=None):
    return {
        "name": name,
        "description": name,
        "inputSchema": {"type": "object", "properties": props or {}, "required": required or []},
        "httpBinding": binding or {"method": "GET", "pathTemplate": f"/{name}", "args": []},
    }


MANIFEST = [
    tool("render_diagram", {"format": {}, "source": {}}),
    tool("list_diagram_types", {"use_case": {}}, binding={"method": "GET", "pathTemplate": "/api/catalog", "args": [{"name": "use_case", "placement": "query"}]}),
    tool("suggest_diagram_type", {"requirements": {}}, ["requirements"], {"method": "POST", "pathTemplate": "/api/catalog/suggest", "args": [{"name": "requirements", "placement": "body"}]}),
    tool(
        "navigate_ui",
        {"session_id": {}, "view": {}, "type_id": {}},
        ["session_id"],
        {"method": "POST", "pathTemplate": "/ui/{session_id}/navigate", "args": [{"name": "session_id", "placement": "path"}, {"name": "type_id", "placement": "query"}, {"name": "view", "placement": "query"}]},
    ),
]


def llm_tools(manifest):
    return [{"type": "function", "function": {"name": t["name"], "description": t["description"], "parameters": t["inputSchema"]}} for t in manifest]


class ToolSelectionTest(unittest.TestCase):
    def names(self, thread):
        return {t["function"]["name"] for t in server.tools_for_thread(llm_tools(MANIFEST) + [server.ask_user_tool], thread)}

    def test_planner_threads_get_only_the_planning_tools(self):
        self.assertEqual(self.names("planner-abc"), {"list_diagram_types", "suggest_diagram_type", "navigate_ui", "ask_user"})

    def test_other_threads_never_get_navigate_ui(self):
        names = self.names("t-1")
        self.assertNotIn("navigate_ui", names)
        self.assertIn("render_diagram", names)
        self.assertIn("list_diagram_types", names)

    def test_session_id_is_hidden_from_the_model_without_mutating_the_manifest(self):
        nav = next(t for t in server.tools_for_thread(llm_tools(MANIFEST), "planner-abc") if t["function"]["name"] == "navigate_ui")
        self.assertNotIn("session_id", nav["function"]["parameters"]["properties"])
        self.assertNotIn("session_id", nav["function"]["parameters"]["required"])
        self.assertIn("session_id", MANIFEST[3]["inputSchema"]["properties"])

    def test_bind_arguments_overrides_any_model_supplied_session(self):
        self.assertEqual(server.bind_arguments("navigate_ui", {"session_id": "victim", "view": "gallery"}, "planner-me")["session_id"], "planner-me")
        self.assertEqual(server.bind_arguments("render_diagram", {"format": "d2"}, "planner-me"), {"format": "d2"})


class PlannerRunTest(unittest.TestCase):
    def setUp(self):
        tmp = tempfile.mkdtemp(prefix="kr0ki-planner-")
        for mod in (server, server.chart_store, server.project_store, server.session_log):
            for attr, sub in (("CHARTS_DIR", "charts"), ("DEBUG_LOG_DIR", "debug")):
                if hasattr(mod, attr):
                    p = patch.object(mod, attr, Path(tmp) / sub)
                    p.start()
                    self.addCleanup(p.stop)
        server.project_store._memory.clear()
        server._pending_questions.clear()
        server._drafts.clear()
        for target, kwargs in (("server.fetch_manifest", {"return_value": MANIFEST}),):
            p = patch(target, **kwargs)
            p.start()
            self.addCleanup(p.stop)
        self.calls = []
        p = patch("server.http_call", side_effect=lambda m, u, d=None, h=None: (self.calls.append((m, u, d)) or ("application/json", b'{"delivered":1,"message":"ok"}')))
        p.start()
        self.addCleanup(p.stop)
        llm = patch("llm_client.OpenAICompatibleClient")
        self.client = llm.start().from_env.return_value
        self.addCleanup(llm.stop)
        httpd = server.ThreadingHTTPServer(("127.0.0.1", 0), server.Handler)
        self.port = httpd.server_address[1]
        threading.Thread(target=httpd.serve_forever, daemon=True).start()
        self.addCleanup(httpd.shutdown)

    def run_thread(self, thread_id, tool_args):
        self.client.chat_completion.side_effect = [
            {"model": "m", "choices": [{"message": {"content": "", "tool_calls": [{"id": "c1", "function": {"name": "navigate_ui", "arguments": json.dumps(tool_args)}}]}}], "usage": {"prompt_tokens": 1, "completion_tokens": 1}},
            {"model": "m", "choices": [{"message": {"content": "Done."}}], "usage": {"prompt_tokens": 1, "completion_tokens": 1}},
        ]
        body = json.dumps({"threadId": thread_id, "runId": "r1", "messages": [{"role": "user", "content": "I need to show database tables"}]}).encode()
        req = urllib.request.Request(f"http://127.0.0.1:{self.port}/run", data=body, headers={"Content-Type": "application/json"})
        with urllib.request.urlopen(req, timeout=10) as resp:
            return resp.read().decode()

    def test_a_planner_run_steers_only_its_own_session_whatever_the_model_asks_for(self):
        self.run_thread("planner-mine", {"session_id": "someone-else", "view": "gallery", "type_id": "class"})
        nav = [c for c in self.calls if "/navigate" in c[1]]
        self.assertEqual(len(nav), 1, self.calls)
        self.assertIn("/ui/planner-mine/navigate", nav[0][1])
        self.assertNotIn("someone-else", nav[0][1])
        self.assertIn("type_id=class", nav[0][1])

    def test_planner_runs_use_the_planner_prompt_and_tool_set(self):
        self.run_thread("planner-mine", {"view": "gallery"})
        messages, tools = self.client.chat_completion.call_args_list[0].args
        self.assertTrue(messages[0]["content"].startswith(server.PLANNER_PREAMBLE))
        self.assertEqual({t["function"]["name"] for t in tools} - {"ask_user"}, {"list_diagram_types", "suggest_diagram_type", "navigate_ui"})

    def test_a_regular_thread_cannot_call_navigate_ui(self):
        self.run_thread("regular-1", {"view": "gallery"})
        self.assertFalse([c for c in self.calls if "/navigate" in c[1]])
        _, tools = self.client.chat_completion.call_args_list[0].args
        self.assertNotIn("navigate_ui", {t["function"]["name"] for t in tools})
        messages, _ = self.client.chat_completion.call_args_list[0].args
        self.assertFalse(messages[0]["content"].startswith(server.PLANNER_PREAMBLE))


if __name__ == "__main__":
    unittest.main()
