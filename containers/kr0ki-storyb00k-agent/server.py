"""AG-UI SSE sidecar for read-only model storytelling and gated draft proposals.

Robustness contract (2026-09-19):
- Multi-round tool loop: internal model/tool turns have their own safety ceiling;
  clarifying questions have a separate per-project user-facing budget.
- Streaming narration: TEXT_MESSAGE_START/CONTENT/END emitted per assistant
  message, so the UI can render text as it arrives.
- Full AG-UI event hygiene: RUN_STARTED → TEXT_MESSAGE_* / TOOL_CALL_* /
  STATE_DELTA / STATE_SNAPSHOT → RUN_FINISHED / RUN_ERROR, with TEXT_MESSAGE_END
  always emitted even when the LLM returns no content.
- Draft proposals are also mirrored into state (`/drafts/-`) so the UI shows
  pending/decided proposals without needing the interrupt payload.
- Token usage is reported to the UI via CUSTOM usage events.
"""

import base64
import json
import os
import sys
import time
import urllib.request
import uuid
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer
from pathlib import Path

import llm_client
from draft_graph import DraftGraph
import chart_store
import project_store
import session_log
try:
    from manifest_dispatch import apply_binding, fetch_manifest, find_tool, http_call
except ModuleNotFoundError:  # local source-tree tests; the image copies the module beside us
    sys.path.insert(0, str(Path(__file__).parents[1] / "kr0ki-mcp"))
    from manifest_dispatch import apply_binding, fetch_manifest, find_tool, http_call

KR0KI_URL = os.environ.get("KR0KI_URL", "http://127.0.0.1:8787")
SKILLS_DIR = Path(__file__).parent / "skills"
MAX_MODEL_TOOL_ROUNDS = int(os.environ.get("KR0KI_STORYB00K_MAX_MODEL_TOOL_ROUNDS", "64"))
MAX_CLARIFYING_QUESTIONS = int(os.environ.get(
    "KR0KI_STORYB00K_MAX_CLARIFYING_QUESTIONS",
    os.environ.get("KR0KI_STORYB00K_MAX_TOOL_ROUNDS", "6"),
))
MAX_OUTPUT_CHARS = int(os.environ.get("KR0KI_STORYB00K_MAX_OUTPUT_CHARS", "20000"))
THREAD_TTL_SECS = int(os.environ.get("KR0KI_STORYB00K_THREAD_TTL_SECS", str(6 * 3600)))
_drafts = {}  # thread_id -> {"graph": DraftGraph, "last_used": epoch}
_pending_questions = {}  # thread_id -> ask_user question awaiting an answer
ALLOWED_ORIGINS = frozenset(
    origin.strip()
    for origin in os.environ.get(
        "KR0KI_STORYB00K_ALLOWED_ORIGINS",
        "http://localhost:8787",
    ).split(",")
    if origin.strip()
)

SYSTEM_PREAMBLE = (
    "You are storyb00k, the planning front-end for a live SysML/Kroki model service "
    "and chart workspace. You work on PROJECTS: one project = one diagram goal the "
    "user is refining with you.\n\n"
    "WORKFLOW — every request goes through a planning stage first:\n"
    "1. THINK: use your thinking channel to restate the goal, list what you know, "
    "and list what is ambiguous (diagram type? scope? level of detail? naming? layout?).\n"
    "2. EXPLORE: internally develop plausible candidate diagrams. Use render tools "
    "to render each useful candidate, request PNG output when supported, and inspect "
    "the returned image before deciding what to refine. Tool calls, thinking, and "
    "candidate comparisons are internal work; they do not count as user questions.\n"
    "3. ASK: after exploring candidates, use ask_user only when a material user "
    "preference or missing requirement remains. Ask one concise multiple-choice "
    "question per interrupt, batch related choices, and never repeat an answered "
    "question. The project-wide maximum is six questions.\n"
    "4. LOCK: once you can restate the user's desire precisely, summarize the "
    "requirements in one short paragraph and call it 'Locked in:' — the user then "
    "sees exactly what will be built. Only after locking in, fetch model facts and "
    "render the diagram.\n"
    "5. FINALIZE: after reviewing candidate renders and applying user answers, "
    "lock the requirements and present the selected result. Prefer tool evidence "
    "over guessing: if you lack a fact, call a tool before answering.\n\n"
    "FAST-TRACK OVERRIDES (user-authorized best judgement — no more questions):\n"
    "- If PROJECT MEMORY contains 'Fast-track: diagram now', or the user's latest "
    "message says 'diagram now', you MUST NOT call ask_user or "
    "recommend_diagram_type again. Pick the best-judgement type and parameters "
    "from everything known so far, state your choices in one short paragraph "
    "(prefixed 'Best judgement:'), treat it as the lock-in, and render immediately "
    "in the same run.\n"
    "- Discovery still applies the max-3-questions budget; from the third question "
    "onward prefer fast-tracking over asking.\n\n"
    "When the user asks for a change to the authoritative model, propose it with the "
    "propose_draft_change tool — never claim to have modified the authoritative "
    "model. Keep narration concise."
)

# Multiple-choice refinement question, surfaced to the user as an interrupt.
ask_user_tool = {
    "type": "function",
    "function": {
        "name": "ask_user",
        "description": (
        "Ask the user ONE multiple-choice question to refine the project "
        "requirements. Prefer asking after internally rendering and inspecting "
        "plausible candidates. The per-project question budget is supplied in "
        "the system prompt."
        ),
        "parameters": {
            "type": "object",
            "required": ["question", "options"],
            "properties": {
                "question": {"type": "string", "description": "The question to put to the user."},
                "options": {
                    "type": "array",
                    "minItems": 2,
                    "maxItems": 6,
                    "items": {"type": "string"},
                    "description": "2–6 concise candidate answers.",
                },
                "allowFreeText": {"type": "boolean", "description": "Whether the user may answer in their own words. Default true."},
            },
        },
    },
}

# Type-discovery tool (Plan 005 §2.2): when the user has NOT named a diagram
# syntax, the agent discovers the right TYPE by intent before rendering.
recommend_diagram_type_tool = {
    "type": "function",
    "function": {
        "name": "recommend_diagram_type",
        "description": (
            "Recommend diagram type(s) for the user's goal. First render and inspect "
            "plausible candidates using render tools; then call this with "
            "mode='confirm' when the user should choose, offering "
            "exactly one primary and (optionally) one alternative type with a "
            "one-line rationale. The user can also pick 'show me both' to see "
            "sample renders of both types side by side."
        ),
        "parameters": {
            "type": "object",
            "required": ["mode", "recommendations"],
            "properties": {
                "mode": {"type": "string", "enum": ["confirm"], "description": "Always 'confirm': commit to recommendations after discovery."},
                "recommendations": {
                    "type": "array",
                    "minItems": 1,
                    "maxItems": 2,
                    "items": {
                        "type": "object",
                        "required": ["typeId", "rationale"],
                        "properties": {
                            "typeId": {"type": "string", "description": "Catalog type id (e.g. 'sequence', 'flowchart')"},
                            "rationale": {"type": "string", "description": "One line: why this type fits the intent."},
                        },
                    },
                },
                "question": {"type": "string", "description": "The confirm question, e.g. 'Go with a sequence diagram?'"},
                "options": {
                    "type": "array",
                    "items": {"type": "string"},
                    "description": "Choices built from the recommended type names plus 'show me both' when two types are offered.",
                },
            },
        },
    },
}


def load_skills():
    return {path.stem: path.read_text() for path in SKILLS_DIR.glob("*.md")}


def thread_draft(thread_id):
    now = time.time()
    # TTL sweep keeps long-lived processes from leaking per-thread graphs.
    stale = [t for t, v in _drafts.items() if now - v["last_used"] > THREAD_TTL_SECS]
    for t in stale:
        _drafts.pop(t, None)
    entry = _drafts.setdefault(thread_id, {"graph": DraftGraph(), "last_used": now})
    entry["last_used"] = now
    return entry["graph"]


def sse_event(event):
    # AG-UI BaseEvent requires type + (threadId, runId) on lifecycle events;
    # extra keys ride on the passthrough `metadata` field.
    return f"data: {json.dumps(event)}\n\n".encode("utf-8")


def lifecycle_event(event_type, thread_id, run_id, **fields):
    """RUN_STARTED/RUN_FINISHED/RUN_ERROR frame with the required threadId."""
    return {"type": event_type, "threadId": thread_id, "runId": run_id, **fields}


def usage_array(prompt_tokens, completion_tokens):
    if not prompt_tokens and not completion_tokens:
        return None
    return [{"inputTokens": prompt_tokens or 0, "outputTokens": completion_tokens or 0,
             "totalTokens": (prompt_tokens or 0) + (completion_tokens or 0)}]


def local_draft_tool():
    return {"type": "function", "function": {"name": "propose_draft_change", "description": "Propose a disposable draft-only change for explicit user approval.", "parameters": {"type": "object", "required": ["subject", "predicate", "object"], "properties": {"subject": {"type": "string"}, "predicate": {"type": "string"}, "object": {"type": "string"}}}}}


def truncate(text, limit=MAX_OUTPUT_CHARS):
    if len(text) <= limit:
        return text
    return text[:limit] + f"\n… [truncated {len(text) - limit} chars]"


def normalize_type_id(value):
    """Normalize a UI label only for comparison with catalog type ids."""
    return "-".join(str(value or "").strip().lower().split())


def selected_recommendation_type(answer, recommendations):
    """Return the canonical recommendation id for a normal confirmation choice."""
    answer_id = normalize_type_id(answer)
    for recommendation in recommendations:
        type_id = normalize_type_id(recommendation.get("typeId"))
        if type_id and type_id == answer_id:
            return recommendation["typeId"]
    return None


def render_recommendation_samples(recommendations):
    """Render the catalog fixtures for a comparison choice without involving the LLM."""
    catalog = json.loads(urllib.request.urlopen(f"{KR0KI_URL}/api/catalog", timeout=5).read())
    examples = json.loads(urllib.request.urlopen(f"{KR0KI_URL}/api/examples", timeout=5).read())
    types = {entry["id"]: entry for entry in catalog.get("types", [])}
    fixtures = {entry["id"]: entry for entry in examples}
    panels = []
    for recommendation in recommendations[:2]:
        type_id = recommendation.get("typeId")
        diagram_type = types.get(type_id, {})
        fixture = fixtures.get(diagram_type.get("exampleId"))
        if not fixture:
            continue
        route = fixture.get("route") or f"/render/{fixture['format']}"
        content_type, result = http_call(
            "POST", f"{KR0KI_URL}{route}?output=svg", fixture["source"].encode()
        )
        panel = panel_from_tool_result(
            "render_diagram", {"format": fixture["format"], "source": fixture["source"]}, content_type, result
        )
        panel["title"] = diagram_type.get("name", type_id)
        panel["source"]["route"] = fixture.get("route")
        panels.append(panel)
    return panels


def panel_from_tool_result(name, arguments, content_type, result):
    """Preserve binary render output rather than corrupting PNG bytes as UTF-8."""
    # Tool-name → editor format/route mapping: the k8s tools take a `manifest`
    # arg and have no `format` argument, so without this the EDIT button would
    # host their YAML on a D2 example and the editor would POST it to
    # /render/d2 → 400. Route-aware so RendererPanel targets the right endpoint.
    tool_format = {
        "render_kubernetes_topology": ("k8s-topology", "/render/k8s-topology"),
        "render_kubernetes_manifest": ("k8s-topology", "/render/k8s-topology"),
        "render_kubediagram": ("kubediagram", "/render/kubediagram"),
    }.get(name)
    if tool_format:
        panel_format, panel_route = tool_format
    else:
        panel_format, panel_route = arguments.get("format"), None
    panel = {
        "kind": "render" if name.startswith("render_") else "query-result",
        "toolName": name,
        "source": {
            "text": arguments.get("source") or arguments.get("manifest"),
            "format": panel_format,
            "route": panel_route,
        } if name.startswith("render_") else None,
    }
    if content_type.startswith("image/") and content_type != "image/svg+xml":
        panel["imageDataUrl"] = f"data:{content_type};base64,{base64.b64encode(result).decode('ascii')}"
        panel["content"] = ""
    else:
        panel["content"] = truncate(result.decode("utf-8", "replace"))
    return panel


def messages_from_payload(payload):
    """Normalize AG-UI messages to OpenAI chat shape; tolerate id-only fields."""
    out = []
    for msg in payload.get("messages") or []:
        role = msg.get("role")
        if not role:
            continue
        item = {"role": role, "content": msg.get("content") or ""}
        if msg.get("tool_calls"):
            item["tool_calls"] = msg["tool_calls"]
        if msg.get("tool_call_id"):
            item["tool_call_id"] = msg["tool_call_id"]
        out.append(item)
    return out


class RunStream:
    """Buffers SSE frames and always closes with RUN_ERROR on failure paths.

    Event shapes follow the @ag-ui/core zod schemas exactly: lifecycle events
    carry threadId; tool results are TOOL_CALL_RESULT events (not TOOL_CALL_END
    extras); usage is an array of TokenUsage objects.
    """

    def __init__(self, handler, run_id, thread_id, run_log=None):
        self.handler = handler
        self.run_id = run_id
        self.thread_id = thread_id
        self.run_log = run_log
        self.message_index = 0

    def write(self, event):
        self.handler.wfile.write(sse_event(event))

    def try_write(self, event):
        try:
            self.write(event)
            if self.run_log is not None:
                self.run_log.agui_event(event)
        except (BrokenPipeError, ConnectionResetError):
            if self.run_log is not None:
                self.run_log.event("client.disconnected", {}, level="warn")

    def text_message(self, content, role="assistant"):
        message_id = f"msg-{self.message_index}"
        self.message_index += 1
        self.try_write({"type": "TEXT_MESSAGE_START", "threadId": self.thread_id, "runId": self.run_id, "messageId": message_id, "role": role})
        self.try_write({"type": "TEXT_MESSAGE_CONTENT", "threadId": self.thread_id, "runId": self.run_id, "messageId": message_id, "delta": content})
        self.try_write({"type": "TEXT_MESSAGE_END", "threadId": self.thread_id, "runId": self.run_id, "messageId": message_id})

    def thinking_message(self, content):
        """Emit the model's chain of thought as THINKING_* events. The
        @ag-ui/client maps these to a `reasoning` part on the assistant item,
        so the transcript can show what the model was thinking."""
        if not content:
            return
        self.try_write({"type": "THINKING_TEXT_MESSAGE_START", "threadId": self.thread_id, "runId": self.run_id})
        self.try_write({"type": "THINKING_TEXT_MESSAGE_CONTENT", "threadId": self.thread_id, "runId": self.run_id, "delta": content})
        self.try_write({"type": "THINKING_TEXT_MESSAGE_END", "threadId": self.thread_id, "runId": self.run_id})


class Handler(BaseHTTPRequestHandler):
    def log_message(self, _format, *_args):
        pass

    def _json(self, status, payload):
        body = json.dumps(payload).encode("utf-8")
        self.send_response(status)
        self.send_header("Content-Type", "application/json")
        self._cors_headers()
        self.send_header("Content-Length", str(len(body)))
        self.end_headers()
        self.wfile.write(body)

    def _cors_headers(self):
        # The playbook is served by kr0ki on :8787 while this sidecar listens on
        # :8789, so the browser requires an explicit local-development CORS bridge.
        origin = self.headers.get("Origin")
        if origin in ALLOWED_ORIGINS:
            self.send_header("Access-Control-Allow-Origin", origin)
            self.send_header("Vary", "Origin")
        self.send_header("Access-Control-Allow-Methods", "GET, POST, OPTIONS")
        self.send_header("Access-Control-Allow-Headers", "Content-Type")

    def do_OPTIONS(self):
        self.send_response(204)
        self._cors_headers()
        self.end_headers()

    def do_GET(self):
        if self.path == "/health":
            self._json(200, {
                "status": "ok",
                "service": "kr0ki-storyb00k-agent",
                "llm_configured": bool(os.environ.get("OPENAI_API_KEY") and os.environ.get("OPENAI_API_URL")),
                "active_threads": len(_drafts),
                "max_model_tool_rounds": MAX_MODEL_TOOL_ROUNDS,
                "max_clarifying_questions": MAX_CLARIFYING_QUESTIONS,
                "debug_log_dir_writable": session_log._DIR_WRITABLE,
            })
        elif self.path == "/debug/sessions":
            self._json(200, {"sessions": session_log.list_sessions()})
        elif self.path == "/charts":
            self._json(200, {"charts": chart_store.list_charts()})
        elif self.path == "/projects":
            self._json(200, {"projects": project_store.list_projects()})
        elif self.path.startswith("/projects/"):
            thread_id = self.path[len("/projects/"):] or "default"
            project = project_store.get_project(thread_id, create=False)
            if project is None:
                return self._json(404, {"error": "project_not_found"})
            self._json(200, project)
        elif self.path.startswith("/charts/history/"):
            thread_id = self.path[len("/charts/history/"):] or "default"
            self._json(200, {"thread": thread_id, "log": chart_store.history(thread_id)})
        elif self.path.startswith("/charts/"):
            thread_id = self.path[len("/charts/"):] or "default"
            graph = chart_store.load_chart(thread_id)
            if graph is None:
                return self._json(404, {"error": "chart_not_found"})
            self._json(200, graph)
        elif self.path.startswith("/debug/sessions/"):
            thread_id = self.path[len("/debug/sessions/"):] or "default"
            snapshot = session_log.get_session_full(thread_id)
            if snapshot is None:
                return self._json(404, {"error": "session_not_found"})
            self._json(200, snapshot)
        elif self.path.startswith("/threads/"):
            thread_id = self.path[len("/threads/"):] or "default"
            draft = _drafts.get(thread_id)
            if draft is None:
                return self._json(404, {"error": "thread_not_found"})
            graph = draft["graph"]
            return self._json(200, {
                "threadId": thread_id,
                "pending": graph.pending_proposals(),
                "turtle": graph.as_turtle(),
            })
        else:
            self._json(404, {"error": "not_found"})

    def do_POST(self):
        try:
            length = int(self.headers.get("Content-Length", 0))
            payload = json.loads(self.rfile.read(length)) if length else {}
        except (ValueError, json.JSONDecodeError):
            return self._json(400, {"error": "invalid_json"})
        if self.path == "/projects/rename":
            thread_id = payload.get("threadId", "default")
            project_store.rename(thread_id, payload.get("title", ""))
            renamed = project_store.get_project(thread_id)
            return self._json(200, {"status": "ok", "title": renamed.get("title") if renamed else None})
        if self.path == "/charts/save":
            try:
                result = chart_store.save_chart(
                    payload.get("threadId", "default"),
                    payload.get("graph"),
                    payload.get("source", ""),
                    payload.get("format", "d2"),
                    description=payload.get("description"),
                )
            except ValueError as error:
                return self._json(400, {"error": str(error)})
            return self._json(200, result)
        if self.path == "/charts/restore":
            thread_id = payload.get("threadId", "default")
            commit_id = payload.get("commitId", "")
            if not commit_id:
                return self._json(400, {"error": "commitId required"})
            return self._json(200, chart_store.restore(thread_id, commit_id))
        if self.path == "/projects/fasttrack":
            thread_id = payload.get("threadId", "default")
            project = project_store.get_project(thread_id)
            if project is not None:
                project["fastTrack"] = bool(payload.get("enabled", True))
                project_store.save_project(thread_id, project)
            return self._json(200, {"status": "ok"})
        if self.path == "/projects/lock-type":
            thread_id = payload.get("threadId", "default")
            type_id = normalize_type_id(payload.get("typeId"))
            if not type_id:
                return self._json(400, {"error": "type_id_required"})
            project = project_store.get_project(thread_id) or {"threadId": thread_id}
            project["lockedType"] = type_id
            project_store.save_project(thread_id, project)
            return self._json(200, project)
        if self.path == "/respond-to-interrupt":
            thread_id = payload.get("threadId", "default")
            draft = _drafts.get(thread_id)
            question = _pending_questions.get(thread_id)
            is_answer = question is not None and payload.get("interruptId") == question.get("id")
            if draft is None and not is_answer:
                return self._json(404, {"error": "draft_session_not_found"})
            if is_answer and question is not None:
                # Planning refinement / type-confirm answer: record it in the
                # project QA history, hand it back to the caller (the UI
                # resumes the run with the answer recorded client-side).
                # Tolerate an approval-style payload ({approved: true}) from a
                # generic client: treat it as choosing the primary option.
                if payload.get("answer") is None and payload.get("approved") is not None:
                    options = question.get("options") or []
                    payload = {**payload, "answer": options[0] if options else "yes"}
                answer = str(payload.get("answer") or "").strip()[:2000]
                if not answer:
                    return self._json(400, {"error": "answer_required"})
                _pending_questions.pop(thread_id, None)
                project_store.add_qa(thread_id, question.get("question", ""), answer)
                if question.get("kind") == "type-confirm":
                    recommendations = question.get("recommendations") or []
                    if normalize_type_id(answer) == "show-me-both":
                        # A comparison is deliberately not a type lock. Render
                        # the catalog fixtures now so the UI can show the
                        # promised visual comparison instead of treating this
                        # label as an invalid diagram type on the next run.
                        try:
                            panels = render_recommendation_samples(recommendations)
                        except Exception as error:  # noqa: BLE001 — answer remains valid if rendering is unavailable
                            return self._json(502, {"error": "comparison_render_failed", "detail": str(error)})
                        return self._json(200, {
                            "status": "ok", "answered": question.get("id"), "question": question.get("question"),
                            "answer": answer, "comparisonTypes": [r.get("typeId") for r in recommendations],
                            "comparisonPanels": panels,
                        })
                    # Lock the canonical typeId from the recommendation, not a
                    # display label supplied by the interrupt client.
                    selected_type = selected_recommendation_type(answer, recommendations)
                    project = project_store.get_project(thread_id)
                    if project is not None and selected_type:
                        project["lockedType"] = selected_type
                        project_store.save_project(thread_id, project)
                return self._json(200, {"status": "ok", "answered": question.get("id"), "question": question.get("question"), "answer": answer})
            try:
                (draft["graph"].apply if payload.get("approved") else draft["graph"].decline)(payload["interruptId"])
            except KeyError:
                return self._json(404, {"error": "proposal_not_found"})
            return self._json(200, {"status": "ok"})
        if self.path != "/run":
            return self._json(404, {"error": "not_found"})

        run_id = payload.get("runId", "unknown")
        thread_id = payload.get("threadId", "default")
        draft = thread_draft(thread_id)
        # Project capture: every user prompt is logged verbatim; the first
        # prompt of a thread becomes the project goal.
        for msg in payload.get("messages") or []:
            if msg.get("role") == "user":
                text = (msg.get("content") or "").strip()
                if text:
                    project_store.add_prompt(thread_id, "user", text, run=run_id)
        self.send_response(200)
        self.send_header("Content-Type", "text/event-stream")
        self.send_header("Cache-Control", "no-cache")
        self._cors_headers()
        self.end_headers()
        stream = RunStream(self, run_id, thread_id)
        run_log = session_log.RunLogger(thread_id, run_id, client_ip=self.client_address[0] if self.client_address else "")
        stream.run_log = run_log
        stream.try_write(lifecycle_event("RUN_STARTED", thread_id, run_id))
        try:
            self._run(stream, payload, thread_id, draft, run_log)
        except Exception as error:  # noqa: BLE001 — the SSE contract wants RUN_ERROR, not a 500
            run_log.error("run", error)
            stream.try_write(lifecycle_event("RUN_ERROR", thread_id, run_id, message=str(error)))
        finally:
            run_log.close()

    # -- the actual agent loop -------------------------------------------------

    def _run(self, stream, payload, thread_id, draft, run_log):
        manifest = fetch_manifest(KR0KI_URL)
        tools = [{"type": "function", "function": {"name": tool["name"], "description": tool["description"], "parameters": tool["inputSchema"]}} for tool in manifest]
        tools.append(local_draft_tool())
        project = project_store.get_project(thread_id) or {}
        questions_asked = int(project.get("questionsAsked", len(project.get("qa", []))))
        questions_remaining = max(0, MAX_CLARIFYING_QUESTIONS - questions_asked)
        if questions_remaining and not project.get("fastTrack"):
            tools.append(ask_user_tool)
            tools.append(recommend_diagram_type_tool)
        # Requirement memory: answered questions and any locked-in summary ride
        # in the system prompt. The client replays the same messages on resume
        # (and fresh sends may drop the answer context entirely), so without
        # this the model re-asks questions the user already answered.
        memory_lines = []
        if project.get("requirements"):
            memory_lines.append(f"Locked in requirements (do not re-litigate): {project['requirements']}")
        for qa in project.get("qa", []):
            memory_lines.append(f"Already answered — Q: {qa['question']} A: {qa['answer']}")
        # Plan 005 §2: two question modes. If no type is locked, discover intent
        # without spending model/tool rounds on user-facing questions.
        if project.get("lockedType"):
            memory_lines.append(f"Type chosen: {project['lockedType']} — do not switch without asking.")
        # Fast-track (Plan 005 UX): after 2+ answered questions the user can
        # authorize best-judgement rendering — the agent must not ask again.
        if project.get("fastTrack"):
            memory_lines.append("Fast-track: diagram now — the user has authorized best judgement; do NOT ask any further questions, render immediately.")
        qa_memory = ""
        if memory_lines:
            qa_memory = (
                "\n\nPROJECT MEMORY (the user has already answered these; NEVER ask "
                "the same or an equivalent question again — treat each answer as a "
                "hard requirement):\n" + "\n".join(f"- {line}" for line in memory_lines)
            )
        # Fast-track (Plan 005 UX) / refine-vs-discover mode selection.
        discovery_mode = not project.get("lockedType")
        try:
            guide = urllib.request.urlopen(f"{KR0KI_URL}/api/catalog", timeout=5).read().decode("utf-8")
            catalog_guide = json.loads(guide).get("discoveryGuide", "")
        except Exception:  # noqa: BLE001 — catalog hiccups must not kill the run
            catalog_guide = ""
        # Fast-track overrides the mode entirely: no questions allowed.
        if project.get("fastTrack"):
            mode_rules = (
                "\n\nQUESTION MODES — FAST-TRACK MODE (user-authorized best judgement): "
                "the user has authorized you to render NOW. Do NOT call ask_user or "
                "recommend_diagram_type — asking is a contract violation. State your "
                "best-judgement choices (prefix 'Best judgement:'), treat it as the "
                "lock-in, and render immediately in this run."
            )
        elif discovery_mode:
            mode_rules = (
                "\n\nQUESTION MODES — pick exactly one per run:\n"
                "- DISCOVER MODE (active now): the user has NOT named a diagram syntax. "
                "Use the goal and catalog vocabulary to generate plausible candidate types, "
                "render each useful candidate as PNG, and inspect the returned images "
                "before asking the user to choose. If an important intent detail cannot "
                "be inferred, ask about what they want to CONVEY, never which syntax. "
                "After exploration, call recommend_diagram_type with one primary and "
                "optionally one alternative, each with a concise rationale. NEVER ask "
                "'which syntax/format do you want?'\n"
                "- REFINE MODE: the type is already locked — refine participants, "
                "scope, and level of detail only.\n"
                + (f"\nTYPE VOCABULARY:\n{catalog_guide}\n" if catalog_guide else "")
            )
        else:
            mode_rules = (
                "\n\nQUESTION MODES — REFINE MODE (active now): the diagram type is locked. "
                "Refine only its participants, scope, and level of detail. Do NOT call "
                "recommend_diagram_type and do not re-enter discovery unless the user asks to switch types."
            )
        if questions_remaining:
            question_budget_rules = (
                f"\n\nCLARIFICATION BUDGET: {questions_remaining} of the project's "
                f"{MAX_CLARIFYING_QUESTIONS} user-facing questions remain. Count only "
                "ask_user and recommend_diagram_type interrupts. Thinking, generating "
                "candidates, rendering, inspecting images, and other internal tool "
                "rounds do not use this budget. Explore candidates before asking."
            )
        else:
            question_budget_rules = (
                "\n\nCLARIFICATION BUDGET EXHAUSTED: do not ask the user another "
                "clarifying or type-confirmation question. Continue autonomously using "
                "best judgement; you may keep generating, rendering, and inspecting "
                "candidate diagrams before presenting the strongest result."
            )
        qa_memory += mode_rules + question_budget_rules
        # Plan 005 §3: per-type skill loaded ONLY when the type is locked —
        # replaces the generic skill dump so context stays small and the syntax
        # guidance matches the chosen diagram.
        locked_type = (project.get("lockedType") or "").strip().lower()
        type_skill_path = SKILLS_DIR / "types" / f"{locked_type}.md"
        if locked_type and type_skill_path.is_file():
            skills_text = type_skill_path.read_text()
        else:
            skills_text = "\n\n".join(load_skills().values())
        messages = [{"role": "system", "content": SYSTEM_PREAMBLE + qa_memory + "\n\n" + skills_text}]
        messages += messages_from_payload(payload)
        # Also inject the answers as an explicit tool-result conversation turn so
        # the model sees them in the message flow, not only the system prompt.
        for qa in project.get("qa", []):
            already = any(
                isinstance(m.get("content"), str) and qa["answer"] in m["content"]
                for m in messages if m.get("role") == "user"
            )
            if not already:
                messages.append({
                    "role": "user",
                    "content": f"(answer to your question \"{qa['question']}\"): {qa['answer']}",
                })
        client = llm_client.OpenAICompatibleClient.from_env()

        usage_total = {"promptTokens": 0, "completionTokens": 0}
        rounds = 0
        while True:
            rounds += 1
            run_log.event("llm.round.start", {"round": rounds})
            response = client.chat_completion(messages, tools)
            usage = response.get("usage") or {}
            usage_total["promptTokens"] += usage.get("prompt_tokens") or 0
            usage_total["completionTokens"] += usage.get("completion_tokens") or 0
            run_log.llm_call(rounds, len(messages), len(tools),
                             usage={"promptTokens": usage.get("prompt_tokens"), "completionTokens": usage.get("completion_tokens")},
                             model=response.get("model"))
            message = response["choices"][0]["message"]
            if message.get("reasoning_content"):
                # Deep-think models expose their chain of thought here; surface
                # it through AG-UI THINKING_* events (rendered by the client as
                # reasoning parts) instead of swallowing it. Also captured in
                # the project log so sessions stay auditable server-side.
                stream.thinking_message(message["reasoning_content"])
                project_store.add_thinking(thread_id, message["reasoning_content"], run=stream.run_id)
            if message.get("content"):
                project_store.add_prompt(thread_id, "assistant", message["content"], run=stream.run_id)
                # Locked-in detection: the workflow contract says the agent
                # prefixes its requirement summary with "Locked in:".
                content_text = message["content"]
                marker = "Locked in:"
                if marker in content_text and not (project_store.get_project(thread_id) or {}).get("locked"):
                    project_store.set_requirements(thread_id, content_text.split(marker, 1)[1].strip())
            messages.append({
                "role": "assistant",
                "content": message.get("content") or "",
                **({"tool_calls": message["tool_calls"]} if message.get("tool_calls") else {}),
            })
            if message.get("content"):
                stream.text_message(message["content"])

            calls = message.get("tool_calls") or []
            if not calls:
                break
            if rounds >= MAX_MODEL_TOOL_ROUNDS:
                stream.text_message(f"[internal tool safety ceiling reached after {rounds} rounds]")
                break

            for call in calls:
                name = call["function"]["name"]
                try:
                    arguments = json.loads(call["function"]["arguments"] or "{}")
                except json.JSONDecodeError:
                    arguments = {}
                tool_call_id = call.get("id", name)
                parent_message_id = f"msg-tool-{tool_call_id}"
                if name in {"ask_user", "recommend_diagram_type"}:
                    if project.get("fastTrack"):
                        allowed = False
                        question_count = int(project.get("questionsAsked", len(project.get("qa", []))))
                    else:
                        allowed, question_count = project_store.record_clarifying_question(
                            thread_id, MAX_CLARIFYING_QUESTIONS
                        )
                    if not allowed:
                        content = "Fast-track is active. Do not ask the user; continue with best judgement." if project.get("fastTrack") else (
                            "The project's clarifying-question budget is exhausted. "
                            "Do not ask the user again; use best judgement and continue "
                            "rendering or inspecting candidates."
                        )
                        stream.try_write({"type": "TOOL_CALL_START", "threadId": thread_id, "runId": stream.run_id, "toolCallId": tool_call_id, "toolCallName": name, "parentMessageId": parent_message_id})
                        stream.try_write({"type": "TOOL_CALL_ARGS", "threadId": thread_id, "runId": stream.run_id, "toolCallId": tool_call_id, "delta": call["function"].get("arguments") or "{}"})
                        stream.try_write({"type": "TOOL_CALL_END", "threadId": thread_id, "runId": stream.run_id, "toolCallId": tool_call_id})
                        stream.try_write({"type": "TOOL_CALL_RESULT", "threadId": thread_id, "runId": stream.run_id, "messageId": parent_message_id, "toolCallId": tool_call_id, "content": content, "role": "tool"})
                        messages.append({"role": "tool", "tool_call_id": tool_call_id, "content": content})
                        run_log.event("plan.question_budget_exhausted", {"tool": name, "questionsAsked": question_count})
                        continue
                if name == "recommend_diagram_type":
                    # Plan 005 §2.2: the agent commits to 1–2 type recommendations.
                    # The confirm answer locks the type into the project.
                    recs = arguments.get("recommendations") or []
                    rec_id = f"rec-{uuid.uuid4().hex[:12]}"
                    stream.try_write({"type": "TOOL_CALL_START", "threadId": thread_id, "runId": stream.run_id, "toolCallId": tool_call_id, "toolCallName": name, "parentMessageId": parent_message_id})
                    stream.try_write({"type": "TOOL_CALL_ARGS", "threadId": thread_id, "runId": stream.run_id, "toolCallId": tool_call_id, "delta": call["function"]["arguments"] or "{}"})
                    stream.try_write({"type": "TOOL_CALL_END", "threadId": thread_id, "runId": stream.run_id, "toolCallId": tool_call_id})
                    stream.try_write({
                        "type": "TOOL_CALL_RESULT", "threadId": thread_id, "runId": stream.run_id,
                        "messageId": parent_message_id, "toolCallId": tool_call_id,
                        "content": f"Recommended: {', '.join(r.get('typeId', '?') for r in recs) or 'none'}", "role": "tool",
                    })
                    _pending_questions[thread_id] = {
                        "id": rec_id,
                        "question": arguments.get("question", "Go with the recommended type?"),
                        "options": arguments.get("options") or [r.get("typeId") for r in recs],
                        "allowFreeText": True,
                        "toolCallId": tool_call_id,
                        "kind": "type-confirm",
                        "recommendations": recs,
                        "ts": time.time(),
                    }
                    run_log.event("plan.recommend", {"id": rec_id, "recommendations": recs})
                    stream.try_write(lifecycle_event("RUN_FINISHED", thread_id, stream.run_id, outcome={
                        "type": "interrupt",
                        "interrupts": [{
                            "id": rec_id,
                            "reason": arguments.get("question", "Go with the recommended type?"),
                            "responseSchema": {
                                "type": "object",
                                "properties": {
                                    "answer": {"type": "string"},
                                    "options": {"type": "array", "items": {"type": "string"}, "default": arguments.get("options") or [r.get("typeId") for r in recs]},
                                },
                                "required": ["answer"],
                            },
                        }],
                    }))
                    return
                if name == "ask_user":
                    # Planning refinement: surface the question as an interrupt.
                    # The client answers via /respond-to-interrupt; the answer
                    # lands in _pending_answers so the next /run call (which
                    # carries the full conversation again) sees it via the
                    # messages the client appends. We park the run here.
                    question_id = f"ask-{uuid.uuid4().hex[:12]}"
                    options = arguments.get("options") or []
                    # Contract: TOOL_CALL_START → ARGS → END → TOOL_CALL_RESULT.
                    # Skipping START/ARGS made the client reject END ("No active
                    # tool call found") and abort the run before the interrupt.
                    stream.try_write({
                        "type": "TOOL_CALL_START", "threadId": thread_id, "runId": stream.run_id,
                        "toolCallId": tool_call_id, "toolCallName": name, "parentMessageId": parent_message_id,
                    })
                    stream.try_write({
                        "type": "TOOL_CALL_ARGS", "threadId": thread_id, "runId": stream.run_id,
                        "toolCallId": tool_call_id, "delta": call["function"]["arguments"] or "{}",
                    })
                    stream.try_write({
                        "type": "TOOL_CALL_END", "threadId": thread_id, "runId": stream.run_id, "toolCallId": tool_call_id,
                    })
                    stream.try_write({
                        "type": "TOOL_CALL_RESULT", "threadId": thread_id, "runId": stream.run_id,
                        "messageId": parent_message_id, "toolCallId": tool_call_id,
                        "content": f"Question put to user: {arguments.get('question', '')}", "role": "tool",
                    })
                    _pending_questions[thread_id] = {
                        "id": question_id,
                        "question": arguments.get("question", ""),
                        "options": options,
                        "allowFreeText": bool(arguments.get("allowFreeText", True)),
                        "toolCallId": tool_call_id,
                        "ts": time.time(),
                    }
                    run_log.event("plan.question", {"id": question_id, "question": arguments.get("question", ""), "options": options})
                    # Interrupt outcome shape per RunFinishedInterruptOutcomeSchema.
                    stream.try_write(lifecycle_event("RUN_FINISHED", thread_id, stream.run_id, outcome={
                        "type": "interrupt",
                        "interrupts": [{
                            "id": question_id,
                            "reason": arguments.get("question", ""),
                            "responseSchema": {
                                "type": "object",
                                "properties": {
                                    "answer": {"type": "string", "description": "Chosen option or free text"},
                                    "optionIndex": {"type": "integer"},
                                    "options": {"type": "array", "items": {"type": "string"}, "default": options, "description": "Candidate answers offered"},
                                    "allowFreeText": {"type": "boolean", "default": bool(arguments.get("allowFreeText", True))},
                                },
                                "required": ["answer"],
                            },
                        }],
                    }))
                    return
                if name == "propose_draft_change":
                    proposal_id = draft.propose(arguments.get("subject", ""), arguments.get("predicate", ""), arguments.get("object", ""))
                    # Contract: the assistant emitted a tool_call, so the client
                    # needs the START → ARGS → END → RESULT lifecycle before the
                    # interrupt, same as ask_user.
                    stream.try_write({"type": "TOOL_CALL_START", "threadId": thread_id, "runId": stream.run_id, "toolCallId": tool_call_id, "toolCallName": name, "parentMessageId": parent_message_id})
                    stream.try_write({"type": "TOOL_CALL_ARGS", "threadId": thread_id, "runId": stream.run_id, "toolCallId": tool_call_id, "delta": call["function"]["arguments"] or "{}"})
                    stream.try_write({"type": "TOOL_CALL_END", "threadId": thread_id, "runId": stream.run_id, "toolCallId": tool_call_id})
                    stream.try_write({
                        "type": "TOOL_CALL_RESULT", "threadId": thread_id, "runId": stream.run_id,
                        "messageId": parent_message_id, "toolCallId": tool_call_id,
                        "content": f"Proposal {proposal_id} pending user approval.", "role": "tool",
                    })
                    stream.try_write({"type": "STATE_DELTA", "threadId": thread_id, "runId": stream.run_id, "delta": [{"op": "add", "path": "/drafts/-", "value": {"id": proposal_id, "status": "pending", **arguments}}]})
                    # Interrupt outcome shape per RunFinishedInterruptOutcomeSchema.
                    stream.try_write(lifecycle_event("RUN_FINISHED", thread_id, stream.run_id, outcome={
                        "type": "interrupt",
                        "interrupts": [{
                            "id": proposal_id,
                            "reason": f"Apply {arguments.get('predicate')}={arguments.get('object')!r} to {arguments.get('subject')}?",
                            "responseSchema": {"type": "object", "properties": {"approved": {"type": "boolean"}}},
                        }],
                    }))
                    return
                tool = find_tool(manifest, name)
                if tool is None:
                    messages.append({"role": "tool", "tool_call_id": tool_call_id, "content": f"unknown tool {name}"})
                    continue
                stream.try_write({"type": "TOOL_CALL_START", "threadId": thread_id, "runId": stream.run_id, "toolCallId": tool_call_id, "toolCallName": name, "parentMessageId": parent_message_id})
                stream.try_write({"type": "TOOL_CALL_ARGS", "threadId": thread_id, "runId": stream.run_id, "toolCallId": tool_call_id, "delta": call["function"]["arguments"] or "{}"})
                started = time.time()
                tool_image_data_url = None
                try:
                    method, url, body = apply_binding(tool, arguments, KR0KI_URL)
                    content_type, result = http_call(method, url, body)
                    panel = panel_from_tool_result(name, arguments, content_type, result)
                    tool_content = panel["content"] or "Rendered binary image."
                    tool_image_data_url = panel.get("imageDataUrl")
                    tool_ok = True
                    run_log.tool_call(name, True, int((time.time() - started) * 1000))
                except Exception as tool_error:  # noqa: BLE001 — feed the failure back to the LLM
                    tool_content = f"tool {name} failed: {tool_error}"
                    tool_ok = False
                    run_log.tool_call(name, False, int((time.time() - started) * 1000), error=tool_error)
                # TOOL_CALL_END carries only ids; results are TOOL_CALL_RESULT.
                stream.try_write({"type": "TOOL_CALL_END", "threadId": thread_id, "runId": stream.run_id, "toolCallId": tool_call_id})
                stream.try_write({"type": "TOOL_CALL_RESULT", "threadId": thread_id, "runId": stream.run_id, "messageId": parent_message_id, "toolCallId": tool_call_id, "content": truncate(tool_content)[:2000], "role": "tool"})
                if tool_ok:
                    stream.try_write({"type": "STATE_DELTA", "threadId": thread_id, "runId": stream.run_id, "delta": [{"op": "add", "path": "/panels/-", "value": panel}]})
                llm_tool_content = truncate(tool_content)[:2000]
                messages.append({"role": "tool", "tool_call_id": tool_call_id, "content": llm_tool_content})
                if tool_image_data_url:
                    messages.append({
                        "role": "user",
                        "content": [
                            {"type": "text", "text": "Internal candidate review: inspect this rendered image for structure, legibility, omissions, and fit to the user's requirements. Use those observations to refine or compare candidates; do not treat this as a new user question."},
                            {"type": "image_url", "image_url": {"url": tool_image_data_url}},
                        ],
                    })

        stream.try_write({"type": "CUSTOM", "threadId": thread_id, "runId": stream.run_id, "name": "usage", "value": usage_total})
        finished = lifecycle_event("RUN_FINISHED", thread_id, stream.run_id)
        if usage := usage_array(usage_total["promptTokens"], usage_total["completionTokens"]):
            finished["usage"] = usage
        stream.try_write(finished)


def main():
    ThreadingHTTPServer(("0.0.0.0", int(os.environ.get("KR0KI_STORYB00K_PORT", "8789"))), Handler).serve_forever()


if __name__ == "__main__":
    main()
