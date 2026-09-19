"""Serialize chart revisions to the local filesystem as a jj (Jujutsu) repo.

The playb00k's revision graph (playbook/src/lib/revisionGraph.js) records every
diagram state; this module is its server-side persistence mirror. Each diagram
thread gets a directory:

    <KR0KI_CHARTS_DIR>/<threadId>/
        chart.json          the revision graph (nodes, activeId) — source of truth
        diagram.<ext>       the ACTIVE diagram source (plain file, always current)
        .jj/                colocated jj+git repo created on first save

Every save snapshots via jj: the working copy is committed automatically
(jj's automatic working-copy commit), giving free time-travel (`jj log`,
`jj restore`, `jj new`) without inventing our own VCS. If jj is not on PATH
we degrade to plain-file writes and say so in the save result.

Security: threadId is sanitized to a filename-safe slug; paths stay inside the
charts root (no traversal); file bodies are length-capped.
"""

import json
import os
import shutil
import subprocess
import threading
from pathlib import Path

CHARTS_DIR = Path(os.environ.get("KR0KI_CHARTS_DIR", "/var/lib/kr0ki/charts"))
MAX_SOURCE_CHARS = 200_000
MAX_GRAPH_CHARS = 1_000_000
_MAX_SLUG = 64

_lock = threading.Lock()
_jj_available = None  # resolved once per process


def slugify(thread_id):
    # Reject path separators outright (they indicate traversal intent) rather
    # than silently rewriting them.
    if "/" in thread_id or "\\" in thread_id or thread_id in ("", ".", ".."):
        raise ValueError(f"thread id contains path separators: {thread_id!r}")
    safe = "".join(c if c.isalnum() or c in "-_." else "_" for c in thread_id)
    return (safe or "default")[:_MAX_SLUG]


def _thread_dir(thread_id):
    root = CHARTS_DIR.resolve()
    target = (root / slugify(thread_id)).resolve()
    # traversal guard: resolved target must stay inside the resolved root
    if not str(target).startswith(str(root) + os.sep):
        raise ValueError(f"thread path escapes charts root: {thread_id!r}")
    return target


def jj_available():
    global _jj_available
    if _jj_available is None:
        try:
            subprocess.run(["jj", "--version"], capture_output=True, check=True, timeout=10)
            _jj_available = True
        except (OSError, subprocess.SubprocessError):
            _jj_available = False
    return _jj_available


def _run_jj(args, cwd):
    return subprocess.run(["jj", *args], cwd=cwd, capture_output=True, text=True, timeout=30)


def _ensure_repo(thread_dir):
    """Colocated jj+git repo on first save; auto-tracks everything after."""
    if (thread_dir / ".jj").is_dir():
        return
    _run_jj(["git", "init", "--colocate"], thread_dir)


def _extension(fmt):
    return {
        "d2": "d2", "mermaid": "mmd", "plantuml": "puml", "c4plantuml": "puml",
        "graphviz": "dot", "k8s-topology": "yaml", "kubediagram": "yaml",
        "vegalite": "json", "vega": "json", "dbml": "dbml", "erd": "erd",
        "structurizr": "dsl", "tikz": "tex", "wireviz": "yaml",
    }.get((fmt or "d2").lower(), "txt")


def save_chart(thread_id, graph_json, active_source, active_format, description=None):
    """Persist the revision graph + active diagram; jj-snapshot if available.

    Returns a dict describing what happened (never raises on jj issues —
    the plain-file write is the floor; jj is the enhancement).
    """
    if not isinstance(graph_json, (str, dict)):
        raise ValueError("graph must be JSON string or object")
    if len(str(graph_json)) > MAX_GRAPH_CHARS:
        raise ValueError("revision graph too large")
    if len(active_source) > MAX_SOURCE_CHARS:
        raise ValueError("diagram source too large")
    if not isinstance(active_format, str):
        raise ValueError("format must be a string")

    result = {"thread": thread_id, "jj": False, "path": None}
    with _lock:
        try:
            thread_dir = _thread_dir(thread_id)
            thread_dir.mkdir(parents=True, exist_ok=True)
            graph_text = graph_json if isinstance(graph_json, str) else json.dumps(graph_json, indent=2)
            (thread_dir / "chart.json").write_text(graph_text, encoding="utf-8")
            ext = _extension(active_format)
            diagram_path = thread_dir / f"diagram.{ext}"
            diagram_path.write_text(active_source, encoding="utf-8")
            result["path"] = str(thread_dir)
            result["diagramFile"] = diagram_path.name

            if jj_available():
                _ensure_repo(thread_dir)
                message = description or f"chart: {thread_id} — {active_format} update"
                auto = _run_jj(["commit", "-m", message], thread_dir)
                if auto.returncode == 0:
                    result["jj"] = True
                else:
                    # jj commit can fail on empty diff (no change) — that's fine.
                    result["jjNote"] = (auto.stderr or auto.stdout).strip()[-300:]
        except Exception as error:  # noqa: BLE001 — persistence must not take down a run
            result["error"] = str(error)[:500]
        if result.get("error") and not result.get("path"):
            # Validation failures (traversal, size) must surface as real errors,
            # not silent no-ops — the HTTP layer maps ValueError → 400.
            raise ValueError(result["error"])
    return result


def load_chart(thread_id):
    """Read back the revision graph (None when the thread was never saved)."""
    thread_dir = _thread_dir(thread_id)
    path = thread_dir / "chart.json"
    if not path.is_file():
        return None
    return json.loads(path.read_text(encoding="utf-8"))


def list_charts():
    """All persisted threads: [{thread, diagramFile, revisions?}]"""
    out = []
    if not CHARTS_DIR.is_dir():
        return out
    for entry in sorted(CHARTS_DIR.iterdir()):
        if not entry.is_dir() or (entry / "chart.json").is_file() is False:
            continue
        try:
            graph = json.loads((entry / "chart.json").read_text(encoding="utf-8"))
            out.append({
                "thread": entry.name,
                "revisions": len(graph.get("nodes", [])),
                "activeId": graph.get("activeId"),
            })
        except (OSError, ValueError):
            continue
    return out


def history(thread_id, limit=20):
    """jj log for a thread (empty when jj unavailable or repo not initialized)."""
    thread_dir = _thread_dir(thread_id)
    if not (thread_dir / ".jj").is_dir() or not jj_available():
        return []
    log = _run_jj(["log", "--no-graph", "-n", str(limit),
                   "-T", 'commit_id.short() ++ " " ++ description.first_line() ++ "\\n"'],
                  thread_dir)
    return [line for line in (log.stdout or "").splitlines() if line.strip()]


def restore(thread_id, commit_id):
    """Time travel on the filesystem: restore chart.json + diagram to a commit."""
    thread_dir = _thread_dir(thread_id)
    if not (thread_dir / ".jj").is_dir():
        return {"ok": False, "note": "no jj repo for this thread"}
    res = _run_jj(["restore", "--from", commit_id], thread_dir)
    return {"ok": res.returncode == 0, "note": (res.stderr or res.stdout).strip()[-300:]}


def delete_chart(thread_id):
    thread_dir = _thread_dir(thread_id)
    if thread_dir.is_dir():
        shutil.rmtree(thread_dir)
        return True
    return False
