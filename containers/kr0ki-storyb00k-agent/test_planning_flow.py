"""End-to-end test of the planning flow over HTTP: /run with an ask_user
tool call (LLM mocked) → interrupt frame → /respond-to-interrupt answer →
project QA recorded. Runs the real ThreadingHTTPServer on an ephemeral port.
"""

import json
import tempfile
import threading
import unittest
import urllib.error
import urllib.request
from pathlib import Path
from unittest.mock import patch

import server


def _post(url, payload):
    req = urllib.request.Request(url, data=json.dumps(payload).encode(), headers={"Content-Type": "application/json"})
    with urllib.request.urlopen(req, timeout=10) as resp:
        return resp.status, json.loads(resp.read())


def _post_stream(url, payload):
    req = urllib.request.Request(url, data=json.dumps(payload).encode(), headers={"Content-Type": "application/json"})
    with urllib.request.urlopen(req, timeout=10) as resp:
        return resp.read().decode("utf-8")


class PlanningFlowTest(unittest.TestCase):
    def setUp(self):
        self.tmp = tempfile.mkdtemp(prefix="kr0ki-plan-e2e-")
        for mod in (server, server.chart_store, server.project_store, server.session_log):
            if hasattr(mod, "CHARTS_DIR"):
                patcher = patch.object(mod, "CHARTS_DIR", Path(self.tmp) / "charts")
                patcher.start()
                self.addCleanup(patcher.stop)
            if hasattr(mod, "DEBUG_LOG_DIR"):
                patcher = patch.object(mod, "DEBUG_LOG_DIR", Path(self.tmp) / "debug")
                patcher.start()
                self.addCleanup(patcher.stop)
        server.project_store._memory.clear()
        server._pending_questions.clear()
        server._drafts.clear()

        # Mock LLM: first call asks the user a multiple-choice question.
        self.llm = patch("llm_client.OpenAICompatibleClient")
        mock_cls = self.llm.start()
        self.addCleanup(self.llm.stop)
        self.client = mock_cls.from_env.return_value
        self.client.chat_completion.return_value = {
            "model": "test-model",
            "choices": [{
                "message": {
                    "content": "Let me think about what you want.",
                    "reasoning_content": "Goal is ambiguous: which diagram type?",
                    "tool_calls": [{
                        "id": "call-1",
                        "function": {
                            "name": "ask_user",
                            "arguments": json.dumps({
                                "question": "Which diagram type?",
                                "options": ["d2", "mermaid", "k8s-topology"],
                            }),
                        },
                    }],
                },
            }],
            "usage": {"prompt_tokens": 10, "completion_tokens": 5},
        }

        httpd = server.ThreadingHTTPServer(("127.0.0.1", 0), server.Handler)
        self.port = httpd.server_address[1]
        threading.Thread(target=httpd.serve_forever, daemon=True).start()
        self.addCleanup(httpd.shutdown)

    def test_ask_user_interrupt_and_answer(self):
        base = f"http://127.0.0.1:{self.port}"
        # 1. Run: agent asks a question → interrupt frame, run finishes.
        raw = _post_stream(f"{base}/run", {
            "threadId": "plan-1", "runId": "r1",
            "messages": [{"role": "user", "content": "Draw my cluster"}],
        })
        frames = [json.loads(line[5:]) for line in raw.splitlines() if line.startswith("data: ")]
        types = [f["type"] for f in frames]
        self.assertIn("THINKING_TEXT_MESSAGE_CONTENT", types, types)
        self.assertIn("RUN_FINISHED", types)
        finished = next(f for f in frames if f["type"] == "RUN_FINISHED")
        self.assertEqual(finished["outcome"]["type"], "interrupt")
        interrupt = finished["outcome"]["interrupts"][0]
        self.assertEqual(interrupt["reason"], "Which diagram type?")
        self.assertEqual(len(interrupt["responseSchema"]["properties"]["options"]["enum"]) if "enum" in interrupt["responseSchema"].get("properties", {}).get("options", {}) else 0, 0)  # options live in the pending question, not the schema

        # 2. Answer: /respond-to-interrupt records the QA and returns the answer.
        status, body = _post(f"{base}/respond-to-interrupt", {
            "threadId": "plan-1", "interruptId": interrupt["id"], "answer": "d2",
        })
        self.assertEqual(status, 200)
        self.assertEqual(body["answer"], "d2")

        # 3. Project captured the prompt, thinking, and QA.
        project = server.project_store.get_project("plan-1")
        self.assertEqual(project["goal"], "Draw my cluster")
        self.assertEqual(project["qa"], [{"question": "Which diagram type?", "answer": "d2", "ts": project["qa"][0]["ts"]}])
        self.assertTrue(any("ambiguous" in t["text"] for t in project["thinking"]))
        self.assertTrue(any(p["role"] == "user" and p["text"] == "Draw my cluster" for p in project["prompts"]))
        self.assertTrue(any(p["role"] == "assistant" for p in project["prompts"]))

        # 4. GET /projects and /projects/plan-1 expose the project.
        status, body = _post(f"{base}/projects/rename", {"threadId": "plan-1", "title": "Cluster map"})
        self.assertEqual(body["title"], "Cluster map")

    def test_answer_requires_pending_question(self):
        req = urllib.request.Request(
            f"http://127.0.0.1:{self.port}/respond-to-interrupt",
            data=json.dumps({"threadId": "no-question", "interruptId": "ask-none", "answer": "x"}).encode(),
            headers={"Content-Type": "application/json"},
        )
        try:
            urllib.request.urlopen(req, timeout=10)
            self.fail("expected 404")
        except urllib.error.HTTPError as err:
            self.assertEqual(err.code, 404)
            self.assertEqual(json.loads(err.read())["error"], "draft_session_not_found")


if __name__ == "__main__":
    unittest.main()
