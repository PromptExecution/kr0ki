import json
import os
import threading
import unittest
from http.client import HTTPConnection
from unittest.mock import patch

import server


class ServerTest(unittest.TestCase):
    def setUp(self):
        server._drafts.clear()
        self.httpd = server.ThreadingHTTPServer(("127.0.0.1", 0), server.Handler)
        self.thread = threading.Thread(target=self.httpd.serve_forever, daemon=True)
        self.thread.start()

    def tearDown(self):
        self.httpd.shutdown()
        self.httpd.server_close()

    def request(self, path, body=None):
        connection = HTTPConnection("127.0.0.1", self.httpd.server_address[1], timeout=5)
        connection.request("POST" if body is not None else "GET", path, body=body, headers={"Content-Type": "application/json"} if body else {})
        return connection.getresponse()

    def test_health(self):
        response = self.request("/health")
        self.assertEqual(response.status, 200)
        self.assertEqual(json.loads(response.read())["status"], "ok")

    def test_options_allows_playbook_cross_origin_requests(self):
        connection = HTTPConnection("127.0.0.1", self.httpd.server_address[1], timeout=5)
        connection.request("OPTIONS", "/run")
        response = connection.getresponse()
        self.assertEqual(response.status, 204)
        self.assertEqual(response.getheader("Access-Control-Allow-Origin"), "http://localhost:8787")

    @patch("server.load_skills", return_value={})
    @patch("server.fetch_manifest", return_value=[])
    @patch("server.llm_client.OpenAICompatibleClient.from_env")
    def test_run_emits_started_text_and_finished(self, client_factory, _manifest, _skills):
        client_factory.return_value.chat_completion.return_value = {"choices": [{"message": {"content": "Hello!", "tool_calls": []}}]}
        response = self.request("/run", json.dumps({"threadId": "t1", "runId": "r1", "messages": [{"role": "user", "content": "hi"}]}))
        body = response.read().decode()
        self.assertEqual(response.status, 200)
        self.assertIn('"type": "RUN_STARTED"', body)
        self.assertIn('"type": "TEXT_MESSAGE_CONTENT"', body)
        self.assertIn('"type": "RUN_FINISHED"', body)

    @patch("server.load_skills", return_value={})
    @patch("server.fetch_manifest", return_value=[])
    @patch("server.llm_client.OpenAICompatibleClient.from_env")
    def test_draft_proposal_emits_interrupt_then_can_be_approved(self, client_factory, _manifest, _skills):
        client_factory.return_value.chat_completion.return_value = {"choices": [{"message": {"tool_calls": [{"function": {"name": "propose_draft_change", "arguments": json.dumps({"subject": "elem-1", "predicate": "name", "object": "Engine v2"})}}]}}]}
        response = self.request("/run", json.dumps({"threadId": "t1", "runId": "r1", "messages": []}))
        event = response.read().decode()
        proposal_id = json.loads(event.split("data: ")[2])["interrupt"]["interrupts"][0]["id"]
        response = self.request("/respond-to-interrupt", json.dumps({"threadId": "t1", "interruptId": proposal_id, "approved": True}))
        self.assertEqual(response.status, 200)
        self.assertIn("Engine v2", server._drafts["t1"].as_turtle())
