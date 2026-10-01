"""Errand: local model (via pi) drafts advanced-syntax examples; kr0ki's real renderer is the judge."""
import json, os, re, subprocess, sys, time, urllib.request, urllib.error
OUT = os.path.dirname(os.path.abspath(__file__))
PI = os.environ["PI_SMOKE_DIR"]  # isolated pi agent dir tree: agent/models.json (provider "local"), work/
mid = json.load(open(f"{PI}/agent/models.json"))["providers"]["local"]["models"][0]["id"]
env = {**os.environ, "PI_CODING_AGENT_DIR": f"{PI}/agent", "PI_OFFLINE": "1", "PI_SKIP_VERSION_CHECK": "1", "PI_TELEMETRY": "0"}
TOPICS = {
 "d2": "styling (fill, stroke, shape: cylinder/person), nested containers, connections with labels and arrowheads, quoting and escaping special characters in labels (colons, dots, quotes, pipes), multi-line labels with |md blocks, classes/variables, sql_table and sequence_diagram shapes",
 "graphviz": "record-shaped nodes with ports and escaping of |{}<> in labels, HTML-like labels, clusters/subgraphs, rank=same, edge ports and compass points, quoting identifiers with spaces and quotes, node/edge default attributes",
 "plantuml": "a class diagram: generics, stereotypes, visibility, abstract/interface, packages, notes, escaping of special characters, skinparam styling, hide empty members",
 "nwdiag": "network segments with addresses, nodes in several networks, groups, colors, labels with special characters, peer networks, node shapes/icons",
}
def ask(fmt, topic):
    prompt = (f"You write reference examples for the {fmt} diagram language rendered by Kroki. Produce 6 SHORT, valid, self-contained "
              f"{fmt} sources, each demonstrating a different advanced or non-obvious feature from this list: {topic}. "
              "Prefer features that people commonly get wrong (escaping, quoting, syntax variants). "
              'Reply with ONLY a JSON array, no prose, no code fences: [{"name": "...", "tip": "one sentence on the gotcha", "source": "..."}]')
    t0 = time.time()
    r = subprocess.run(["pi", "--offline", "--no-session", "--provider", "local", "--model", mid, "--no-tools", "--no-extensions", "--no-skills", "--no-context-files", "-p", prompt],
                       cwd=f"{PI}/work", env=env, stdin=subprocess.DEVNULL, capture_output=True, text=True, timeout=420)
    return r.stdout, time.time() - t0
def parse(text):
    m = re.search(r"\[.*\]", text, re.S)
    return json.loads(m.group(0)) if m else []
def render(fmt, src):
    req = urllib.request.Request(f"http://127.0.0.1:8787/render/{fmt}?output=svg", data=src.encode(), headers={"Content-Type": "text/plain"})
    try:
        with urllib.request.urlopen(req, timeout=60) as r:
            return r.status, ""
    except urllib.error.HTTPError as e:
        return e.code, e.read().decode("utf-8", "replace")[:160]
    except Exception as e:
        return 0, str(e)[:160]
results = {}
for fmt, topic in TOPICS.items():
    try:
        out, secs = ask(fmt, topic)
        items = parse(out)
    except Exception as e:
        results[fmt] = {"error": str(e)[:200]}; print(fmt, "ERR", e, flush=True); continue
    rows = []
    for it in items:
        st, err = render(fmt, it.get("source", ""))
        rows.append({"name": it.get("name"), "ok": st == 200, "status": st, "err": err, "tip": it.get("tip"), "source": it.get("source")})
    results[fmt] = {"secs": round(secs), "n": len(rows), "ok": sum(r["ok"] for r in rows), "rows": rows}
    print(fmt, results[fmt]["ok"], "/", results[fmt]["n"], f"{secs:.0f}s", flush=True)
json.dump(results, open(f"{OUT}/results.json", "w"), indent=1)
