import json
import unittest
from unittest.mock import patch

import server
import session_log


class SessionLogTest(unittest.TestCase):
    def setUp(self):
        session_log._sessions.clear()
        session_log._DIR_WRITABLE = False  # keep tests off the real filesystem

    def test_run_logger_records_events_errors_and_closes(self):
        log = session_log.RunLogger("t-log", "r-log")
        log.event("custom.thing", {"k": "v"})
        log.tool_call("list_formats", True, 12)
        log.llm_call(1, 3, 14, usage={"promptTokens": 5, "completionTokens": 2}, model="m")
        try:
            raise ValueError("boom")
        except ValueError as error:
            log.error("unit-test", error)
        log.close()
        snapshot = session_log.get_session_full("t-log")
        self.assertEqual(snapshot["runs"], 1)
        self.assertEqual(snapshot["errorCount"], 1)
        kinds = [e["kind"] for e in snapshot["events"]]
        self.assertIn("run.open", kinds)
        self.assertIn("custom.thing", kinds)
        self.assertIn("tool.executed", kinds)
        self.assertIn("llm.call", kinds)
        self.assertIn("error", kinds)
        self.assertIn("run.close", kinds)
        self.assertIn("boom", snapshot["errors"][0]["error"])
        self.assertIn("Traceback", snapshot["errors"][0]["traceback"])

    def test_sessions_are_bounded(self):
        for i in range(session_log.MAX_SESSIONS + 5):
            session_log.get_session(f"thread-{i}")
        self.assertLessEqual(len(session_log.list_sessions()), session_log.MAX_SESSIONS)


class DebugEndpointsTest(unittest.TestCase):
    def setUp(self):
        session_log._sessions.clear()
        self.httpd = server.ThreadingHTTPServer(("127.0.0.1", 0), server.Handler)
        threading = __import__("threading")
        self.thread = threading.Thread(target=self.httpd.serve_forever, daemon=True)
        self.thread.start()

    def tearDown(self):
        self.httpd.shutdown()
        self.httpd.server_close()

    def request(self, path):
        from http.client import HTTPConnection
        conn = HTTPConnection("127.0.0.1", self.httpd.server_address[1], timeout=5)
        conn.request("GET", path)
        resp = conn.getresponse()
        return resp.status, json.loads(resp.read())

    def test_debug_sessions_lists_active_threads(self):
        session_log.get_session("t-debug-list")  # ensure at least one exists
        status, body = self.request("/debug/sessions")
        self.assertEqual(status, 200)
        ids = [s["threadId"] for s in body["sessions"]]
        self.assertIn("t-debug-list", ids)

    def test_debug_session_detail_404s_for_unknown_thread(self):
        status, body = self.request("/debug/sessions/nope")
        self.assertEqual(status, 404)

    @patch("server.load_skills", return_value={})
    @patch("server.fetch_manifest", return_value=[])
    @patch("server.llm_client.OpenAICompatibleClient.from_env")
    def test_run_produces_queryable_session(self, client_factory, _manifest, _skills):
        client_factory.return_value.chat_completion.return_value = {
            "choices": [{"message": {"content": "hi", "tool_calls": []}}]}
        from http.client import HTTPConnection
        conn = HTTPConnection("127.0.0.1", self.httpd.server_address[1], timeout=5)
        conn.request("POST", "/run", body=json.dumps({"threadId": "t-debug-run", "runId": "r-debug", "messages": []}),
                     headers={"Content-Type": "application/json"})
        conn.getresponse().read()
        status, snapshot = self.request("/debug/sessions/t-debug-run")
        self.assertEqual(status, 200)
        self.assertEqual(snapshot["runs"], 1)
        kinds = [e["kind"] for e in snapshot["events"]]
        self.assertIn("agui.RUN_STARTED", kinds)
        self.assertIn("agui.RUN_FINISHED", kinds)


if __name__ == "__main__":
    unittest.main()
