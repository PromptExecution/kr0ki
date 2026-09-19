"""AG-UI SSE sidecar for read-only model storytelling and gated draft proposals.

Robustness contract (2026-09-19):
- Multi-round tool loop: the LLM may keep calling tools after seeing results
  (bounded by MAX_TOOL_ROUNDS) instead of the old single round-trip.
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
MAX_TOOL_ROUNDS = int(os.environ.get("KR0KI_STORYB00K_MAX_TOOL_ROUNDS", "6"))
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
    "2. ASK: if anything material is ambiguous, use the ask_user tool to put ONE "
    "multiple-choice question to the user. Each answer refines the shared "
    "understanding. Ask as many rounds as needed — but batch what you can and never "
    "re-ask something already answered.\n"
    "3. LOCK: once you can restate the user's desire precisely, summarize the "
    "requirements in one short paragraph and call it 'Locked in:' — the user then "
    "sees exactly what will be built. Only after locking in, fetch model facts and "
    "render the diagram.\n"
    "4. RENDER: use the tools to read the live model and render. Prefer tool "
    "evidence over guessing: if you lack a fact, call a tool before answering.\n\n"
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
            "requirements. Use this during planning whenever the goal is ambiguous; "
            "keep asking until you can state the desire precisely (locked in)."
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
                "max_tool_rounds": MAX_TOOL_ROUNDS,
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
        if self.path == "/respond-to-interrupt":
            thread_id = payload.get("threadId", "default")
            draft = _drafts.get(thread_id)
            question = _pending_questions.get(thread_id)
            is_answer = question is not None and payload.get("interruptId") == question.get("id")
            if draft is None and not is_answer:
                return self._json(404, {"error": "draft_session_not_found"})
            if is_answer and question is not None:
                # Planning refinement answer: record it in the project QA
                # history, hand it back to the caller (the UI immediately
                # re-runs with the answer appended as a user message,
                # continuing the planning loop).
                answer = str(payload.get("answer") or "").strip()[:2000]
                if not answer:
                    return self._json(400, {"error": "answer_required"})
                _pending_questions.pop(thread_id, None)
                project_store.add_qa(thread_id, question.get("question", ""), answer)
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
        tools.append(ask_user_tool)
        messages = [{"role": "system", "content": SYSTEM_PREAMBLE + "\n\n" + "\n\n".join(load_skills().values())}]
        messages += messages_from_payload(payload)
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
            if rounds >= MAX_TOOL_ROUNDS:
                stream.text_message(f"[tool budget exhausted after {rounds} rounds — stopping; ask me to continue]")
                break

            for call in calls:
                name = call["function"]["name"]
                try:
                    arguments = json.loads(call["function"]["arguments"] or "{}")
                except json.JSONDecodeError:
                    arguments = {}
                tool_call_id = call.get("id", name)
                parent_message_id = f"msg-tool-{tool_call_id}"
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
                try:
                    method, url, body = apply_binding(tool, arguments, KR0KI_URL)
                    content_type, result = http_call(method, url, body)
                    panel = panel_from_tool_result(name, arguments, content_type, result)
                    tool_content = panel["content"] or "Rendered binary image."
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
                    messages.append({"role": "tool", "tool_call_id": tool_call_id, "content": tool_content})
                else:
                    messages.append({"role": "tool", "tool_call_id": tool_call_id, "content": tool_content})

        stream.try_write({"type": "CUSTOM", "threadId": thread_id, "runId": stream.run_id, "name": "usage", "value": usage_total})
        finished = lifecycle_event("RUN_FINISHED", thread_id, stream.run_id)
        if usage := usage_array(usage_total["promptTokens"], usage_total["completionTokens"]):
            finished["usage"] = usage
        stream.try_write(finished)


def main():
    ThreadingHTTPServer(("0.0.0.0", int(os.environ.get("KR0KI_STORYB00K_PORT", "8789"))), Handler).serve_forever()


if __name__ == "__main__":
    main()
