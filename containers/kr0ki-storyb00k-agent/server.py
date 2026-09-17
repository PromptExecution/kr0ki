"""AG-UI SSE sidecar for read-only model storytelling and gated draft proposals."""

import base64
import json
import os
import sys
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer
from pathlib import Path

import llm_client
from draft_graph import DraftGraph
try:
    from manifest_dispatch import apply_binding, fetch_manifest, find_tool, http_call
except ModuleNotFoundError:  # local source-tree tests; the image copies the module beside us
    sys.path.insert(0, str(Path(__file__).parents[1] / "kr0ki-mcp"))
    from manifest_dispatch import apply_binding, fetch_manifest, find_tool, http_call

KR0KI_URL = os.environ.get("KR0KI_URL", "http://127.0.0.1:8787")
SKILLS_DIR = Path(__file__).parent / "skills"
_drafts = {}
ALLOWED_ORIGINS = frozenset(
    origin.strip()
    for origin in os.environ.get(
        "KR0KI_STORYB00K_ALLOWED_ORIGINS",
        "http://localhost:8787",
    ).split(",")
    if origin.strip()
)


def load_skills():
    return {path.stem: path.read_text() for path in SKILLS_DIR.glob("*.md")}


def sse_event(event):
    return f"data: {json.dumps(event)}\n\n".encode("utf-8")


def local_draft_tool():
    return {"type": "function", "function": {"name": "propose_draft_change", "description": "Propose a disposable draft-only change for explicit user approval.", "parameters": {"type": "object", "required": ["subject", "predicate", "object"], "properties": {"subject": {"type": "string"}, "predicate": {"type": "string"}, "object": {"type": "string"}}}}}


def panel_from_tool_result(name, arguments, content_type, result):
    """Preserve binary render output rather than corrupting PNG bytes as UTF-8."""
    panel = {
        "kind": "render" if name.startswith("render_") else "query-result",
        "toolName": name,
        "source": {
            "text": arguments.get("source") or arguments.get("manifest"),
            "format": arguments.get("format"),
        } if name.startswith("render_") else None,
    }
    if content_type.startswith("image/") and content_type != "image/svg+xml":
        panel["imageDataUrl"] = f"data:{content_type};base64,{base64.b64encode(result).decode('ascii')}"
        panel["content"] = ""
    else:
        panel["content"] = result.decode("utf-8", "replace")
    return panel


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
            self._json(200, {"status": "ok", "service": "kr0ki-storyb00k-agent"})
        else:
            self._json(404, {"error": "not_found"})

    def do_POST(self):
        length = int(self.headers.get("Content-Length", 0))
        payload = json.loads(self.rfile.read(length)) if length else {}
        if self.path == "/respond-to-interrupt":
            draft = _drafts.get(payload.get("threadId", "default"))
            if draft is None:
                return self._json(404, {"error": "draft_session_not_found"})
            try:
                (draft.apply if payload.get("approved") else draft.decline)(payload["interruptId"])
            except KeyError:
                return self._json(404, {"error": "proposal_not_found"})
            return self._json(200, {"status": "ok"})
        if self.path != "/run":
            return self._json(404, {"error": "not_found"})

        run_id = payload.get("runId", "unknown")
        thread_id = payload.get("threadId", "default")
        draft = _drafts.setdefault(thread_id, DraftGraph())
        self.send_response(200)
        self.send_header("Content-Type", "text/event-stream")
        self.send_header("Cache-Control", "no-cache")
        self._cors_headers()
        self.end_headers()
        self.wfile.write(sse_event({"type": "RUN_STARTED", "runId": run_id}))
        try:
            manifest = fetch_manifest(KR0KI_URL)
            tools = [{"type": "function", "function": {"name": tool["name"], "description": tool["description"], "parameters": tool["inputSchema"]}} for tool in manifest]
            tools.append(local_draft_tool())
            messages = [{"role": "system", "content": "\n\n".join(load_skills().values())}] + payload.get("messages", [])
            client = llm_client.OpenAICompatibleClient.from_env()
            message = client.chat_completion(messages, tools)["choices"][0]["message"]
            follow_up_messages = messages + [message]
            for call in message.get("tool_calls") or []:
                name, arguments = call["function"]["name"], json.loads(call["function"]["arguments"])
                if name == "propose_draft_change":
                    proposal_id = draft.propose(arguments["subject"], arguments["predicate"], arguments["object"])
                    self.wfile.write(sse_event({"type": "RUN_FINISHED", "runId": run_id, "interrupt": {"type": "interrupt", "interrupts": [{"id": proposal_id, "reason": f"Apply {arguments['predicate']}={arguments['object']!r} to {arguments['subject']}?", "responseSchema": {"type": "object", "properties": {"approved": {"type": "boolean"}}}}]}}))
                    return
                tool = find_tool(manifest, name)
                if tool is None:
                    continue
                self.wfile.write(sse_event({"type": "TOOL_CALL_START", "runId": run_id, "toolCallId": call.get("id", name), "toolName": name, "args": arguments}))
                method, url, body = apply_binding(tool, arguments, KR0KI_URL)
                content_type, result = http_call(method, url, body)
                panel = panel_from_tool_result(name, arguments, content_type, result)
                self.wfile.write(sse_event({"type": "TOOL_CALL_END", "runId": run_id, "toolCallId": call.get("id", name), "toolName": name, "result": panel["content"]}))
                self.wfile.write(sse_event({"type": "STATE_DELTA", "runId": run_id, "delta": [{"op": "add", "path": "/panels/-", "value": panel}]}))
                follow_up_messages.append({"role": "tool", "tool_call_id": call.get("id", name), "content": panel["content"] or "Rendered binary image."})
            if message.get("tool_calls"):
                message = client.chat_completion(follow_up_messages, tools=[])["choices"][0]["message"]
            if message.get("content"):
                self.wfile.write(sse_event({"type": "TEXT_MESSAGE_CONTENT", "runId": run_id, "role": "assistant", "content": message["content"]}))
            self.wfile.write(sse_event({"type": "RUN_FINISHED", "runId": run_id}))
        except Exception as error:
            self.wfile.write(sse_event({"type": "RUN_ERROR", "runId": run_id, "message": str(error)}))


def main():
    ThreadingHTTPServer(("0.0.0.0", int(os.environ.get("KR0KI_STORYB00K_PORT", "8789"))), Handler).serve_forever()


if __name__ == "__main__":
    main()
