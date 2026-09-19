"""Session observability for the storyb00k agent (operator request, 2026-09-19).

The browser console only shows the client's view. The AGENT needs its own
server-side record of every session so an observer (human or another agent)
can see exactly what happened: events emitted, tool calls, LLM round-trips,
errors with tracebacks.

Three sinks, all thread-safe:

1. Structured stdout lines (`kubectl logs kr0ki-local -c kr0ki-storyb00k-agent`)
   — every event, prefixed with thread/run ids.
2. In-memory session records (bounded) served over HTTP:
   GET /debug/sessions          -> list of thread summaries
   GET /debug/sessions/<id>     -> full event log + errors for one thread
3. JSONL transcript files (one per thread) under KR0KI_DEBUG_LOG_DIR when that
   directory is writable — durable across restarts, greppable, mountable.

Secrets policy: API keys and request bodies are NEVER logged; LLM prompts/
responses are logged at length-capped size.
"""

import json
import os
import threading
import time
import traceback
from collections import OrderedDict
from pathlib import Path

DEBUG_LOG_DIR = Path(os.environ.get("KR0KI_DEBUG_LOG_DIR", "/tmp/kr0ki-storyb00k-debug"))
MAX_SESSIONS = int(os.environ.get("KR0KI_DEBUG_MAX_SESSIONS", "32"))
MAX_EVENTS_PER_SESSION = int(os.environ.get("KR0KI_DEBUG_MAX_EVENTS", "500"))
MAX_BODY_LOG_CHARS = int(os.environ.get("KR0KI_DEBUG_MAX_BODY_CHARS", "4000"))

_lock = threading.Lock()
_sessions = OrderedDict()  # thread_id -> session dict (bounded, oldest evicted)


def _now():
    return time.strftime("%Y-%m-%dT%H:%M:%S", time.gmtime()) + "Z"


class SessionLog:
    """Per-thread record of one or more runs. All methods are exception-safe:
    logging must never take down a run."""

    def __init__(self, thread_id):
        self.thread_id = thread_id
        self.created_at = _now()
        self.runs = 0
        self.errors = []
        self.events = []  # [(ts, kind, detail-dict)]

    def snapshot(self, full=False):
        data = {
            "threadId": self.thread_id,
            "createdAt": self.created_at,
            "runs": self.runs,
            "errorCount": len(self.errors),
            "eventCount": len(self.events),
        }
        if full:
            data["errors"] = self.errors
            data["events"] = [{"ts": ts, "kind": kind, **detail} for ts, kind, detail in self.events]
        return data


def _dir_writable(path):
    try:
        path.mkdir(parents=True, exist_ok=True)
        probe = path / ".probe"
        probe.write_text("ok")
        probe.unlink()
        return True
    except OSError:
        return False


_DIR_WRITABLE = None  # resolved lazily per process


def _jsonl_path(thread_id):
    safe = "".join(c if c.isalnum() or c in "-_." else "_" for c in thread_id) or "default"
    return DEBUG_LOG_DIR / f"thread-{safe}.jsonl"


def get_session(thread_id):
    with _lock:
        session = _sessions.get(thread_id)
        if session is None:
            session = SessionLog(thread_id)
            _sessions[thread_id] = session
            while len(_sessions) > MAX_SESSIONS:
                _sessions.popitem(last=False)
        return session


def list_sessions():
    with _lock:
        return [s.snapshot() for s in _sessions.values()]


def get_session_full(thread_id):
    with _lock:
        session = _sessions.get(thread_id)
        return session.snapshot(full=True) if session else None


class RunLogger:
    """One agent run. `event()` for AG-UI frames, `llm_call()` for round-trips,
    `error()` for failures with traceback. Fans out to stdout, memory, JSONL."""

    def __init__(self, thread_id, run_id, client_ip=""):
        self.session = get_session(thread_id)
        self.thread_id = thread_id
        self.run_id = run_id
        self.session.runs += 1
        self._file = None
        global _DIR_WRITABLE
        if _DIR_WRITABLE is None:
            _DIR_WRITABLE = _dir_writable(DEBUG_LOG_DIR)
        if _DIR_WRITABLE:
            try:
                self._file = open(_jsonl_path(thread_id), "a", encoding="utf-8")
            except OSError:
                self._file = None
        self.event("run.open", {"clientIp": client_ip})

    def _record(self, kind, detail, level="info"):
        ts = _now()
        entry = {"ts": ts, "thread": self.thread_id, "run": self.run_id, "level": level, "kind": kind, **detail}
        # 1. stdout — structured, one line, always
        try:
            print(json.dumps(entry, ensure_ascii=False), flush=True)
        except Exception:
            pass
        # 2. in-memory session (bounded)
        with _lock:
            self.session.events.append((ts, kind, detail))
            if len(self.session.events) > MAX_EVENTS_PER_SESSION:
                del self.session.events[: len(self.session.events) - MAX_EVENTS_PER_SESSION]
        # 3. JSONL file (best effort)
        if self._file:
            try:
                self._file.write(json.dumps(entry, ensure_ascii=False) + "\n")
                self._file.flush()
            except Exception:
                pass

    def event(self, kind, detail=None, level="info"):
        self._record(kind, detail or {}, level)

    def agui_event(self, event):
        """Log an AG-UI SSE frame about to be sent to the browser."""
        self.event(f"agui.{event.get('type', 'unknown')}", _cap(event))

    def llm_call(self, round_no, messages_len, tools_len, usage=None, model=None):
        self.event("llm.call", {
            "round": round_no,
            "messagesInThread": messages_len,
            "toolsOffered": tools_len,
            "model": model,
            "usage": usage,
        })

    def tool_call(self, name, ok, latency_ms, error=None):
        self.event("tool.executed", {"tool": name, "ok": ok, "latencyMs": latency_ms, "error": None if error is None else str(error)[:500]},
                   level="info" if ok else "error")

    def error(self, where, error):
        detail = {
            "where": where,
            "error": str(error)[:1000],
            "traceback": traceback.format_exc(limit=8)[-2000:],
        }
        self._record("error", detail, level="error")
        with _lock:
            self.session.errors.append({"ts": _now(), "run": self.run_id, **detail})

    def close(self):
        if self._file:
            try:
                self._file.close()
            except Exception:
                pass
            self._file = None
        self.event("run.close", {})


def _cap(obj, limit=MAX_BODY_LOG_CHARS):
    """Length-cap any JSON value for logging (prompts/responses can be large)."""
    try:
        text = json.dumps(obj, ensure_ascii=False)
    except (TypeError, ValueError):
        return {"unserializable": True}
    if len(text) <= limit:
        return obj
    return {"truncated": text[:limit], "originalChars": len(text)}
