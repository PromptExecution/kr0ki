"""Per-thread PROJECT state for storyb00k.

A project is the conceptual unit the user works on within a session:
    {
      "threadId": ...,
      "title": short name (first prompt, editable),
      "goal": the user's initial desire,
      "requirements": "Locked in:" summary the agent produced (None until locked),
      "qa": [ {"question", "answer", "ts"} ... ],   # refinement history
      "prompts": [ {"role", "text", "ts", "run"} ... ],  # full prompt log
      "thinking": [ {"text", "ts", "run"} ... ],    # LLM reasoning captured server-side
      "createdAt", "updatedAt",
    }

Persistence: JSON file beside the chart store (<KR0KI_CHARTS_DIR>/<thread>/project.json)
so a project and its charts travel together and survive restarts. Also kept in
memory for cheap reads. Nothing here talks to the LLM.
"""

import json
import os
import threading
import time
from pathlib import Path

CHARTS_DIR = Path(os.environ.get("KR0KI_CHARTS_DIR", "/var/lib/kr0ki/charts"))
MAX_TEXT = 20_000
MAX_LOG = 500  # entries per list

_lock = threading.Lock()
_memory = {}  # thread_id -> project dict


def _thread_dir(thread_id):
    from chart_store import slugify
    root = CHARTS_DIR.resolve()
    target = (root / slugify(thread_id)).resolve()
    if not str(target).startswith(str(root) + os.sep):
        raise ValueError(f"thread path escapes charts root: {thread_id!r}")
    return target


def _path(thread_id):
    return _thread_dir(thread_id) / "project.json"


def get_project(thread_id, create=True):
    """Load (or lazily create) the project for a thread."""
    with _lock:
        project = _memory.get(thread_id)
        if project is None:
            path = _path(thread_id)
            if path.is_file():
                try:
                    project = json.loads(path.read_text(encoding="utf-8"))
                except (OSError, ValueError):
                    project = None
            if project is None:
                if not create:
                    return None
                project = {
                    "threadId": thread_id,
                    "title": "Untitled project",
                    "goal": None,
                    "requirements": None,
                    "locked": False,
                    "qa": [],
                    "prompts": [],
                    "thinking": [],
                    "createdAt": time.time(),
                    "updatedAt": time.time(),
                }
            _memory[thread_id] = project
        assert project is not None
        return project


def save_project(thread_id, project):
    project["updatedAt"] = time.time()
    with _lock:
        _memory[thread_id] = project
        try:
            path = _path(thread_id)
            path.parent.mkdir(parents=True, exist_ok=True)
            path.write_text(json.dumps(project, indent=2, ensure_ascii=False), encoding="utf-8")
        except (OSError, ValueError):
            pass  # in-memory copy remains authoritative for the live session
    return project


def set_goal(thread_id, goal):
    project = get_project(thread_id)
    if project.get("goal") is None:
        project["goal"] = goal[:MAX_TEXT]
        project["title"] = goal.strip().splitlines()[0][:80] or "Untitled project"
    return save_project(thread_id, project)


def add_prompt(thread_id, role, text, run=None):
    """Full prompt/response log — user prompts, assistant narration, verbatim."""
    project = get_project(thread_id)
    entry = {"role": role, "text": (text or "")[:MAX_TEXT], "ts": time.time()}
    if run:
        entry["run"] = run
    project["prompts"].append(entry)
    del project["prompts"][:-MAX_LOG]
    if role == "user":
        set_goal(thread_id, text or "")
    else:
        save_project(thread_id, project)
    return entry


def add_thinking(thread_id, text, run=None):
    """Capture the LLM's reasoning server-side (always, even if the browser
    never renders it) so sessions are fully auditable."""
    project = get_project(thread_id)
    entry = {"text": (text or "")[:MAX_TEXT], "ts": time.time()}
    if run:
        entry["run"] = run
    project["thinking"].append(entry)
    del project["thinking"][:-MAX_LOG]
    return save_project(thread_id, project)


def add_qa(thread_id, question, answer):
    project = get_project(thread_id)
    project["qa"].append({"question": question[:MAX_TEXT], "answer": answer[:MAX_TEXT], "ts": time.time()})
    del project["qa"][:-MAX_LOG]
    return save_project(thread_id, project)


def set_requirements(thread_id, requirements):
    project = get_project(thread_id)
    project["requirements"] = (requirements or "")[:MAX_TEXT]
    project["locked"] = bool(requirements)
    return save_project(thread_id, project)


def rename(thread_id, title):
    project = get_project(thread_id)
    project["title"] = (title or "").strip()[:120] or project["title"]
    return save_project(thread_id, project)


def list_projects():
    out = []
    if not CHARTS_DIR.is_dir():
        return out
    for entry in sorted(CHARTS_DIR.iterdir()):
        path = entry / "project.json"
        if entry.is_dir() and path.is_file():
            try:
                p = json.loads(path.read_text(encoding="utf-8"))
                out.append({
                    "threadId": p.get("threadId", entry.name),
                    "title": p.get("title"),
                    "goal": p.get("goal"),
                    "locked": p.get("locked", False),
                    "qaCount": len(p.get("qa", [])),
                    "promptCount": len(p.get("prompts", [])),
                    "updatedAt": p.get("updatedAt"),
                })
            except (OSError, ValueError):
                continue
    return out
