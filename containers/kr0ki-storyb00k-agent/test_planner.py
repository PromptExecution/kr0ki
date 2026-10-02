"""Planner mode: the gallery's planning agent may steer *its own* browser tab and nothing else."""

import base64
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
        self.assertNotIn("view", nav["function"]["parameters"]["properties"])  # the planner never changes the page
        self.assertIn("session_id", MANIFEST[3]["inputSchema"]["properties"])

    def test_bind_arguments_overrides_any_model_supplied_session(self):
        bound = server.bind_arguments("navigate_ui", {"session_id": "victim", "view": "gallery", "type_id": "erd"}, "planner-me")
        self.assertEqual(bound, {"session_id": "planner-me", "type_id": "erd"})  # own session, and no view change
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
        self.assertFalse([c for c in self.calls if "view=" in c[1]], self.calls)
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


import urllib.error as _ue
import io as _io


class ToolErrorTest(unittest.TestCase):
    def http_error(self, code, body):
        return _ue.HTTPError("http://x", code, "Bad", {}, _io.BytesIO(body.encode()))

    def test_http_errors_carry_the_servers_own_message(self):
        e = self.http_error(422, json.dumps({"error": "bad_source", "message": "Error 400: Syntax Error? (Assumed diagram type: sequence)"}))
        self.assertEqual(server.describe_tool_error(e), "HTTP 422: Error 400: Syntax Error? (Assumed diagram type: sequence)")

    def test_non_json_bodies_are_collapsed_and_bounded(self):
        e = self.http_error(500, "line one\n\n  line two " + "x" * 2000)
        out = server.describe_tool_error(e, limit=50)
        self.assertTrue(out.startswith("HTTP 500: line one line two"))
        self.assertLessEqual(len(out), len("HTTP 500: ") + 50)

    def test_empty_body_falls_back_to_the_status_line_and_other_errors_pass_through(self):
        self.assertEqual(server.describe_tool_error(self.http_error(400, "")), "HTTP 400 Bad")
        self.assertEqual(server.describe_tool_error(ValueError("boom")), "boom")

    def test_call_signature_ignores_key_order_but_not_content(self):
        a = server.call_signature("render_diagram", {"format": "d2", "source": "a->b"})
        self.assertEqual(a, server.call_signature("render_diagram", {"source": "a->b", "format": "d2"}))
        self.assertNotEqual(a, server.call_signature("render_diagram", {"format": "d2", "source": "a->c"}))


class DuplicateRenderTest(PlannerRunTest):
    def setUp(self):
        super().setUp()
        import tempfile
        # isolate from the shipped skills so the language gate does not hold back the first render here
        p = patch.object(server, "LANGUAGE_SKILLS_DIR", Path(tempfile.mkdtemp(prefix="kr0ki-noskills-")))
        p.start()
        self.addCleanup(p.stop)

    def run_two_identical_renders(self):
        call = {"id": "c1", "function": {"name": "render_diagram", "arguments": json.dumps({"format": "d2", "source": "a -> b"})}}
        call2 = {"id": "c2", "function": {"name": "render_diagram", "arguments": json.dumps({"format": "d2", "source": "a -> b"})}}
        self.client.chat_completion.side_effect = [
            {"model": "m", "choices": [{"message": {"content": "", "tool_calls": [call]}}], "usage": {"prompt_tokens": 1, "completion_tokens": 1}},
            {"model": "m", "choices": [{"message": {"content": "", "tool_calls": [call2]}}], "usage": {"prompt_tokens": 1, "completion_tokens": 1}},
            {"model": "m", "choices": [{"message": {"content": "Done."}}], "usage": {"prompt_tokens": 1, "completion_tokens": 1}},
        ]
        body = json.dumps({"threadId": "regular-9", "runId": "r9", "messages": [{"role": "user", "content": "draw a to b"}]}).encode()
        req = urllib.request.Request(f"http://127.0.0.1:{self.port}/run", data=body, headers={"Content-Type": "application/json"})
        with urllib.request.urlopen(req, timeout=10) as resp:
            return resp.read().decode()

    def test_an_identical_successful_render_is_refused_not_repeated(self):
        raw = self.run_two_identical_renders()
        renders = [c for c in self.calls if c[1].endswith("/render_diagram")]
        self.assertEqual(len(renders), 1, self.calls)
        self.assertIn("already rendered exactly this source", raw)

    # the inherited planner tests are not re-run here
    test_a_planner_run_steers_only_its_own_session_whatever_the_model_asks_for = None
    test_planner_runs_use_the_planner_prompt_and_tool_set = None
    test_a_regular_thread_cannot_call_navigate_ui = None


class LanguageSkillTest(unittest.TestCase):
    def setUp(self):
        import tempfile
        self.tmp = Path(tempfile.mkdtemp(prefix="kr0ki-skills-"))
        (self.tmp / "d2").mkdir()
        (self.tmp / "d2" / "SKILL.md").write_text("---\nname: d2\ndescription: D2 syntax\n---\n# D2\nKeys are identifiers; put names in labels.\n")
        p = patch.object(server, "LANGUAGE_SKILLS_DIR", self.tmp)
        p.start()
        self.addCleanup(p.stop)

    def test_frontmatter_is_stripped_and_missing_or_unsafe_names_yield_nothing(self):
        self.assertTrue(server.language_skill("d2").startswith("# D2"))
        self.assertNotIn("description:", server.language_skill("D2"))
        for bad in ("nope", "../d2", "d2/../d2", "", None, 7):
            self.assertIsNone(server.language_skill(bad), bad)


class SkillGateRunTest(DuplicateRenderTest):
    """A render in a language that has a skill is held back once, with the guide, then allowed."""

    def setUp(self):
        super().setUp()
        import tempfile
        tmp = Path(tempfile.mkdtemp(prefix="kr0ki-skills-"))
        (tmp / "d2").mkdir()
        (tmp / "d2" / "SKILL.md").write_text("---\nname: d2\ndescription: x\n---\n# D2 guide\nUse |md for markdown labels.\n")
        p = patch.object(server, "LANGUAGE_SKILLS_DIR", tmp)
        p.start()
        self.addCleanup(p.stop)

    test_an_identical_successful_render_is_refused_not_repeated = None  # inherited; the gate changes call 1

    def run_calls(self, calls):
        self.client.chat_completion.side_effect = [
            {"model": "m", "choices": [{"message": {"content": "", "tool_calls": [{"id": f"c{i}", "function": {"name": "render_diagram", "arguments": json.dumps(a)}}]}}], "usage": {"prompt_tokens": 1, "completion_tokens": 1}}
            for i, a in enumerate(calls)
        ] + [{"model": "m", "choices": [{"message": {"content": "Done."}}], "usage": {"prompt_tokens": 1, "completion_tokens": 1}}]
        body = json.dumps({"threadId": "regular-gate", "runId": "rg", "messages": [{"role": "user", "content": "draw"}]}).encode()
        req = urllib.request.Request(f"http://127.0.0.1:{self.port}/run", data=body, headers={"Content-Type": "application/json"})
        with urllib.request.urlopen(req, timeout=10) as resp:
            return resp.read().decode()

    def renders(self):
        return [c for c in self.calls if c[1].endswith("/render_diagram")]

    def test_first_render_is_held_back_and_the_guide_reaches_the_model_then_the_retry_runs(self):
        raw = self.run_calls([{"format": "d2", "source": "a -> b"}, {"format": "d2", "source": "a -> b: x"}])
        self.assertEqual(len(self.renders()), 1, self.calls)  # only the retry rendered
        self.assertIn("Loaded the d2 syntax guide", raw)  # the user sees a short note, not the whole guide
        self.assertNotIn("Use |md for markdown labels", raw)
        second_round_messages = self.client.chat_completion.call_args_list[1].args[0]
        tool_msgs = [m for m in second_round_messages if m.get("role") == "tool"]
        # (the mock keeps a reference to the growing message list, so the gate's reply is the FIRST tool message)
        self.assertIn("Use |md for markdown labels", tool_msgs[0]["content"])  # but the model gets it all
        self.assertIn("was NOT run yet", tool_msgs[0]["content"])

    def test_the_guide_is_delivered_once_per_run_and_languages_without_a_skill_are_not_gated(self):
        self.run_calls([{"format": "d2", "source": "a"}, {"format": "d2", "source": "b"}, {"format": "d2", "source": "c"}, {"format": "graphviz", "source": "digraph{a->b}"}])
        self.assertEqual(len(self.renders()), 3)  # d2 gated once; d2 x2 and graphviz ran

    def test_gating_does_not_consume_the_consecutive_failure_budget(self):
        server_budget = server.MAX_CONSECUTIVE_FAILURES
        # one gate hold + (budget - 1) real failures must still leave room for the successful final render
        self.assertGreaterEqual(server_budget, 2)
        raw = self.run_calls([{"format": "d2", "source": "a"}, {"format": "d2", "source": "b"}])
        self.assertNotIn("consecutive tool failures", raw)
        self.assertEqual(len(self.renders()), 1)


class ShippedSkillsTest(unittest.TestCase):
    """Offline shape checks on the real skills/diagrams/*. Rendering every example needs a Kroki backend:
    run tools/skill-pilot/verify_skills.py for that."""

    def test_every_shipped_skill_is_well_formed_and_fits_the_gate(self):
        root = server.SKILLS_DIR / "diagrams"
        langs = sorted(p.name for p in root.iterdir() if p.is_dir())
        self.assertTrue({"d2", "graphviz", "plantuml", "nwdiag"} <= set(langs), langs)
        for fmt in langs:
            raw = (root / fmt / "SKILL.md").read_text()
            self.assertTrue(raw.startswith("---\nname: kr0ki-"), fmt)
            self.assertIn("description: Use before", raw, fmt)
            self.assertLessEqual(len(raw), server.MAX_SKILL_CHARS, f"{fmt} would be truncated by the gate")
            self.assertGreaterEqual(raw.count(f"```{fmt}\n"), 3, f"{fmt} needs worked examples")
            self.assertIn("## Identifiers", raw, fmt)
            self.assertIn("Never invent identifiers", raw, fmt)
            body = server.language_skill(fmt)
            self.assertTrue(body.startswith(f"# {fmt}"), fmt)


class TypeSkillCoverageTest(unittest.TestCase):
    """Every diagram type in the Rust catalog has a type skill with the required sections (quality guidance)."""

    SECTIONS = ["## Choose it when", "## Not when", "## Anatomy", "## What makes it good", "## What makes it bad", "## Questions to ask", "## Contrast"]

    def test_every_catalog_type_has_a_well_formed_type_skill(self):
        import re
        catalog = (Path(__file__).parents[2] / "crates/kr0ki-core/src/catalog.rs").read_text()
        ids = re.findall(r'^\s{8}id: "([a-z0-9-]+)",', catalog, re.M)
        self.assertGreaterEqual(len(ids), 20, ids)
        for tid in ids:
            path = server.SKILLS_DIR / "types" / f"{tid}.md"
            self.assertTrue(path.is_file(), f"no type skill for catalog type '{tid}'")
            text = path.read_text()
            for section in self.SECTIONS:
                self.assertIn(section, text, f"{tid} lacks '{section}'")
            self.assertLessEqual(len(text), 4500, tid)
            self.assertGreaterEqual(len(re.findall(r"```[a-z0-9-]+\n", text)), 2, f"{tid} needs a bad and a good example")


class TextOnlyModelRunTest(DuplicateRenderTest):
    """A PNG render on a text-only model must not put an image in the conversation (the server would answer HTTP 500)."""

    test_an_identical_successful_render_is_refused_not_repeated = None

    def run_png_render(self, vision):
        self.client.vision_enabled.return_value = vision
        png = base64.b64encode(b"\x89PNG fake").decode()
        with patch("server.http_call", side_effect=lambda m, u, d=None, h=None: self.calls.append((m, u, d)) or ("image/png", b"\x89PNG fake")):
            self.client.chat_completion.side_effect = [
                {"model": "m", "choices": [{"message": {"content": "", "tool_calls": [{"id": "c1", "function": {"name": "render_diagram", "arguments": json.dumps({"format": "graphviz", "source": "digraph{a->b}", "output": "png"})}}]}}], "usage": {"prompt_tokens": 1, "completion_tokens": 1}},
                {"model": "m", "choices": [{"message": {"content": "Done."}}], "usage": {"prompt_tokens": 1, "completion_tokens": 1}},
            ]
            body = json.dumps({"threadId": "regular-vis", "runId": "rv", "messages": [{"role": "user", "content": "draw"}]}).encode()
            req = urllib.request.Request(f"http://127.0.0.1:{self.port}/run", data=body, headers={"Content-Type": "application/json"})
            with urllib.request.urlopen(req, timeout=10) as resp:
                resp.read()
        sent = self.client.chat_completion.call_args_list[1].args[0]
        return [m for m in sent if isinstance(m.get("content"), list) and any(p.get("type") == "image_url" for p in m["content"])], sent

    def test_no_image_is_sent_to_a_text_only_model_and_the_model_is_told_why(self):
        images, sent = self.run_png_render(vision=False)
        self.assertEqual(images, [])
        self.assertTrue(any("cannot view images" in str(m.get("content")) for m in sent))

    def test_a_vision_model_still_gets_the_image(self):
        images, _ = self.run_png_render(vision=True)
        self.assertEqual(len(images), 1)
