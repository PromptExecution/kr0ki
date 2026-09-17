"""AG-UI SSE sidecar for read-only model storytelling and gated draft proposals."""

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


def load_skills():
    return {path.stem: path.read_text() for path in SKILLS_DIR.glob("*.md")}


def sse_event(event):
    return f"data: {json.dumps(event)}\n\n".encode("utf-8")


def local_draft_tool():
    return {"type": "function", "function": {"name": "propose_draft_change", "description": "Propose a disposable draft-only change for explicit user approval.", "parameters": {"type": "object", "required": ["subject", "predicate", "object"], "properties": {"subject": {"type": "string"}, "predicate": {"type": "string"}, "object": {"type": "string"}}}}}


class Handler(BaseHTTPRequestHandler):
    def log_message(self, _format, *_args):
        pass

    def _json(self, status, payload):
        body = json.dumps(payload).encode("utf-8")
        self.send_response(status)
        self.send_header("Content-Type", "application/json")
        self.send_header("Content-Length", str(len(body)))
        self.end_headers()
        self.wfile.write(body)

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
        self.end_headers()
        self.wfile.write(sse_event({"type": "RUN_STARTED", "runId": run_id}))
        try:
            manifest = fetch_manifest(KR0KI_URL)
            tools = [{"type": "function", "function": {"name": tool["name"], "description": tool["description"], "parameters": tool["inputSchema"]}} for tool in manifest]
            tools.append(local_draft_tool())
            messages = [{"role": "system", "content": "\n\n".join(load_skills().values())}] + payload.get("messages", [])
            message = llm_client.OpenAICompatibleClient.from_env().chat_completion(messages, tools)["choices"][0]["message"]
            for call in message.get("tool_calls") or []:
                name, arguments = call["function"]["name"], json.loads(call["function"]["arguments"])
                if name == "propose_draft_change":
                    proposal_id = draft.propose(arguments["subject"], arguments["predicate"], arguments["object"])
                    self.wfile.write(sse_event({"type": "RUN_FINISHED", "runId": run_id, "interrupt": {"type": "interrupt", "interrupts": [{"id": proposal_id, "reason": f"Apply {arguments['predicate']}={arguments['object']!r} to {arguments['subject']}?", "responseSchema": {"type": "object", "properties": {"approved": {"type": "boolean"}}}}]}}))
                    return
                tool = find_tool(manifest, name)
                if tool is None:
                    continue
                method, url, body = apply_binding(tool, arguments, KR0KI_URL)
                _, result = http_call(method, url, body)
                self.wfile.write(sse_event({"type": "STATE_DELTA", "runId": run_id, "delta": [{"op": "add", "path": "/panels/-", "value": {"kind": "render" if name.startswith("render_") else "query-result", "toolName": name, "content": result.decode("utf-8", "replace"), "source": arguments if name.startswith("render_") else None}}]}))
            if message.get("content"):
                self.wfile.write(sse_event({"type": "TEXT_MESSAGE_CONTENT", "runId": run_id, "role": "assistant", "content": message["content"]}))
            self.wfile.write(sse_event({"type": "RUN_FINISHED", "runId": run_id}))
        except Exception as error:
            self.wfile.write(sse_event({"type": "RUN_ERROR", "runId": run_id, "message": str(error)}))


def main():
    ThreadingHTTPServer(("0.0.0.0", int(os.environ.get("KR0KI_STORYB00K_PORT", "8789"))), Handler).serve_forever()


if __name__ == "__main__":
    main()
