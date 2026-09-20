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

        # This suite runs the real HTTP server end-to-end but must not depend
        # on a live kr0ki-server being reachable at KR0KI_URL: fetch_manifest()
        # is the one real network call _run() makes before it ever reaches the
        # (also mocked) LLM. Same pattern test_server.py already uses.
        self.manifest = patch("server.fetch_manifest", return_value=[])
        self.manifest.start()
        self.addCleanup(self.manifest.stop)

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
        # Tool-call lifecycle must be complete before the interrupt — the
        # client's state machine rejects END/RESULT without a prior START.
        self.assertEqual(
            [t for t in types if t.startswith("TOOL_CALL")],
            ["TOOL_CALL_START", "TOOL_CALL_ARGS", "TOOL_CALL_END", "TOOL_CALL_RESULT"],
            types,
        )
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

    def test_repeat_run_does_not_reask_answered_questions(self):
        """After a QA round, the next run's LLM context carries the answer —
        in the system prompt memory and as an explicit user turn — so the
        agent cannot re-ask the same question."""
        base = f"http://127.0.0.1:{self.port}"
        # Round 1: the agent asks (mock returns ask_user tool call).
        _post_stream(f"{base}/run", {
            "threadId": "plan-mem", "runId": "r1",
            "messages": [{"role": "user", "content": "Draw my cluster"}],
        })
        # Answer via the interrupt endpoint.
        _post(f"{base}/respond-to-interrupt", {
            "threadId": "plan-mem",
            "interruptId": server._pending_questions["plan-mem"]["id"],
            "answer": "d2",
        })
        # Round 2: capture the messages the LLM receives on the resumed run.
        captured = {}

        def capture_completion(messages, tools):
            captured["messages"] = messages
            return {"model": "t", "choices": [{"message": {"content": "Locked in: a d2 diagram."}}], "usage": {}}

        self.client.chat_completion.side_effect = capture_completion
        _post_stream(f"{base}/run", {
            "threadId": "plan-mem", "runId": "r2",
            "messages": [{"role": "user", "content": "Draw my cluster"}],
        })
        system = captured["messages"][0]["content"]
        self.assertIn("PROJECT MEMORY", system)
        self.assertIn("Already answered", system)
        self.assertIn("d2", system)
        user_turns = [m["content"] for m in captured["messages"] if m["role"] == "user"]
        self.assertTrue(any('(answer to your question "Which diagram type?")' in t and "d2" in t for t in user_turns),
                        user_turns)

    def test_discover_mode_recommends_and_locks_type(self):
        """No syntax named → agent runs discovery, recommends via the tool,
        and the confirm answer locks the type in the project."""
        base = f"http://127.0.0.1:{self.port}"
        # Round 1: discovery question (mock ask_user).
        self.client.chat_completion.side_effect = None
        self.client.chat_completion.return_value = {
            "model": "t",
            "choices": [{
                "message": {
                    "content": "",
                    "tool_calls": [{
                        "id": "c-disc",
                        "function": {
                            "name": "ask_user",
                            "arguments": json.dumps({
                                "question": "Who is this diagram for?",
                                "options": ["developers", "managers", "mixed"],
                            }),
                        },
                    }],
                },
            }],
            "usage": {},
        }
        raw = _post_stream(f"{base}/run", {
            "threadId": "plan-disc", "runId": "r1",
            "messages": [{"role": "user", "content": "I want to show how our release process works"}],
        })
        frames = [json.loads(l[5:]) for l in raw.splitlines() if l.startswith("data: {")]
        interrupt = next(f for f in frames if f["type"] == "RUN_FINISHED")["outcome"]["interrupts"][0]
        _post(f"{base}/respond-to-interrupt", {
            "threadId": "plan-disc", "interruptId": interrupt["id"], "answer": "mixed",
        })
        # Round 2: the agent recommends a type via recommend_diagram_type.
        self.client.chat_completion.return_value = {
            "model": "t",
            "choices": [{
                "message": {
                    "content": "",
                    "tool_calls": [{
                        "id": "c-rec",
                        "function": {
                            "name": "recommend_diagram_type",
                            "arguments": json.dumps({
                                "mode": "confirm",
                                "recommendations": [
                                    {"typeId": "activity", "rationale": "process flow with swimlanes"},
                                    {"typeId": "flowchart", "rationale": "simpler branch view"},
                                ],
                                "question": "Go with an activity diagram?",
                                "options": ["activity", "flowchart", "show me both"],
                            }),
                        },
                    }],
                },
            }],
            "usage": {},
        }
        raw = _post_stream(f"{base}/run", {
            "threadId": "plan-disc", "runId": "r2",
            "messages": [{"role": "user", "content": "I want to show how our release process works"}],
        })
        frames = [json.loads(l[5:]) for l in raw.splitlines() if l.startswith("data: {")]
        rec = next(f for f in frames if f["type"] == "RUN_FINISHED")["outcome"]["interrupts"][0]
        # Options ride the responseSchema default (zod-stripped otherwise).
        self.assertEqual(rec["responseSchema"]["properties"]["options"]["default"], ["activity", "flowchart", "show me both"])
        # Confirm → type locked into the project.
        _post(f"{base}/respond-to-interrupt", {
            "threadId": "plan-disc", "interruptId": rec["id"], "answer": "activity",
        })
        project = server.project_store.get_project("plan-disc")
        self.assertEqual(project["lockedType"], "activity")
        # Round 3: system prompt carries the locked type → refine mode.
        captured = {}

        def capture(messages, tools):
            captured["system"] = messages[0]["content"]
            return {"model": "t", "choices": [{"message": {"content": "Locked in: activity diagram of the release process."}}], "usage": {}}

        self.client.chat_completion.side_effect = capture
        _post_stream(f"{base}/run", {
            "threadId": "plan-disc", "runId": "r3",
            "messages": [{"role": "user", "content": "I want to show how our release process works"}],
        })
        self.assertIn("REFINE MODE", captured["system"])
        self.assertIn("Type chosen: activity", captured["system"])
        self.assertNotIn("DISCOVER MODE (active now)", captured["system"])

    def test_type_confirmation_uses_canonical_recommendation_id_and_compares_both(self):
        base = f"http://127.0.0.1:{self.port}"
        recommendations = [
            {"typeId": "activity", "rationale": "workflow"},
            {"typeId": "flowchart", "rationale": "simpler flow"},
        ]
        server._pending_questions["plan-choice"] = {
            "id": "rec-choice", "kind": "type-confirm", "question": "Choose",
            "options": ["Activity", "Flowchart", "Show me both"], "recommendations": recommendations,
        }
        status, _ = _post(f"{base}/respond-to-interrupt", {
            "threadId": "plan-choice", "interruptId": "rec-choice", "answer": "Activity",
        })
        self.assertEqual(status, 200)
        self.assertEqual(server.project_store.get_project("plan-choice")["lockedType"], "activity")

        server._pending_questions["plan-both"] = {
            "id": "rec-both", "kind": "type-confirm", "question": "Choose",
            "options": ["Activity", "Flowchart", "Show me both"], "recommendations": recommendations,
        }
        with patch.object(server, "render_recommendation_samples", return_value=[{"kind": "render"}, {"kind": "render"}]) as render:
            status, body = _post(f"{base}/respond-to-interrupt", {
                "threadId": "plan-both", "interruptId": "rec-both", "answer": "show me both",
            })
        self.assertEqual(status, 200)
        render.assert_called_once_with(recommendations)
        self.assertEqual(body["comparisonTypes"], ["activity", "flowchart"])
        self.assertEqual(len(body["comparisonPanels"]), 2)
        self.assertNotIn("lockedType", server.project_store.get_project("plan-both"))

    def test_approval_payload_on_question_tolerated(self):
        """A generic client that posts {approved: true} to a rec-* question
        gets the primary option as the answer instead of a 400."""
        base = f"http://127.0.0.1:{self.port}"
        self.client.chat_completion.side_effect = None
        self.client.chat_completion.return_value = {
            "model": "t",
            "choices": [{
                "message": {
                    "content": "",
                    "tool_calls": [{
                        "id": "c-rec2",
                        "function": {
                            "name": "recommend_diagram_type",
                            "arguments": json.dumps({
                                "mode": "confirm",
                                "recommendations": [{"typeId": "flowchart", "rationale": "r"}],
                                "question": "Go with a flowchart?",
                                "options": ["Flowchart", "Show me both"],
                            }),
                        },
                    }],
                },
            }],
            "usage": {},
        }
        _post_stream(f"{base}/run", {
            "threadId": "plan-tol", "runId": "r1",
            "messages": [{"role": "user", "content": "draw it"}],
        })
        status, body = _post(f"{base}/respond-to-interrupt", {
            "threadId": "plan-tol",
            "interruptId": server._pending_questions["plan-tol"]["id"],
            "approved": True,
        })
        self.assertEqual(status, 200)
        self.assertEqual(body["answer"], "Flowchart")
        project = server.project_store.get_project("plan-tol")
        self.assertEqual(project["lockedType"], "flowchart")

    def test_fast_track_skips_further_questions(self):
        """POST /projects/fasttrack arms best-judgement mode: the next run's
        system prompt carries the fast-track override and the answered QA."""
        base = f"http://127.0.0.1:{self.port}"
        status, _ = _post(f"{base}/projects/fasttrack", {"threadId": "plan-ft", "enabled": True})
        self.assertEqual(status, 200)
        captured = {}

        def capture(messages, tools):
            captured["system"] = messages[0]["content"]
            return {"model": "t", "choices": [{"message": {"content": "Best judgement: a flowchart."}}], "usage": {}}

        self.client.chat_completion.side_effect = capture
        _post_stream(f"{base}/run", {
            "threadId": "plan-ft", "runId": "r1",
            "messages": [{"role": "user", "content": "diagram now"}],
        })
        self.assertIn("Fast-track: diagram now", captured["system"])
        self.assertIn("MUST NOT call ask_user", captured["system"])
        self.assertIn("FAST-TRACK MODE", captured["system"])

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
