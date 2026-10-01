"""Skill-corpus generator: the local model (via pi) drafts examples, kr0ki's renderer judges them, and failures are
fed back WITH the renderer's real error for up to REPAIRS attempts. Only examples that render are kept.
Env: PI_SMOKE_DIR (isolated pi dir tree, see pilot.py), KR0KI_URL (default http://127.0.0.1:8787).
Usage: python3 generate.py <out.json> <lang> [<lang> ...]   (lang: d2 graphviz plantuml nwdiag)"""
import json, os, re, subprocess, sys, time, urllib.error, urllib.request

PI = os.environ["PI_SMOKE_DIR"]
KR0KI = os.environ.get("KR0KI_URL", "http://127.0.0.1:8787")
REPAIRS = 2
mid = json.load(open(f"{PI}/agent/models.json"))["providers"]["local"]["models"][0]["id"]
env = {**os.environ, "PI_CODING_AGENT_DIR": f"{PI}/agent", "PI_OFFLINE": "1", "PI_SKIP_VERSION_CHECK": "1", "PI_TELEMETRY": "0"}

TOPICS = {
    "d2": ("d2", "styling (fill, stroke, shape: cylinder/person), nested containers, labelled connections and arrowheads, quoting/escaping special characters in keys and labels, multi-line labels, classes/variables, sql_table, sequence_diagram"),
    "graphviz": ("graphviz", "record-shaped nodes with ports and escaping of |{}<> , HTML-like labels, clusters/subgraphs, rank=same, edge ports, quoting identifiers with spaces, default node/edge attributes"),
    "plantuml": ("plantuml", "class diagrams: generics, stereotypes, visibility, abstract/interface, packages, notes, escaping, skinparam, hide empty members."),
    "nwdiag": ("nwdiag", "network segments with addresses, a node in several networks, groups, colours, labels with special characters, peer networks, node shapes"),
}

def pi(prompt):
    r = subprocess.run(["pi", "--offline", "--no-session", "--provider", "local", "--model", mid, "--no-tools", "--no-extensions",
                        "--no-skills", "--no-context-files", "-p", prompt], cwd=f"{PI}/work", env=env, stdin=subprocess.DEVNULL,
                       capture_output=True, text=True, timeout=420)
    return r.stdout

def json_of(text, opener):
    m = re.search(r"\[.*\]" if opener == "[" else r"\{.*\}", text, re.S)
    try:
        return json.loads(m.group(0)) if m else None
    except ValueError:
        return None

def render(fmt, src):
    req = urllib.request.Request(f"{KR0KI}/render/{fmt}?output=svg", data=src.encode(), headers={"Content-Type": "text/plain"})
    try:
        with urllib.request.urlopen(req, timeout=60) as r:
            return True, ""
    except urllib.error.HTTPError as e:
        body = e.read().decode("utf-8", "replace")
        try: body = json.loads(body).get("message", body)
        except ValueError: pass
        return False, " ".join(body.split())[:300]
    except Exception as e:
        return False, str(e)[:200]

def run(lang):
    fmt, topic = TOPICS[lang]
    draft = json_of(pi(f"You write reference examples for the {lang} diagram language rendered by Kroki. Produce 8 SHORT, valid, self-contained {lang} sources, each demonstrating a different advanced or non-obvious feature from: {topic}. Prefer features people commonly get wrong. Reply with ONLY a JSON array, no prose, no code fences: [{{\"name\":\"...\",\"tip\":\"one sentence gotcha\",\"source\":\"...\"}}]"), "[") or []
    kept, log = [], []
    for ex in draft:
        src, attempts, ok, err = ex.get("source", ""), 0, False, ""
        while True:
            ok, err = render(fmt, src)
            log.append({"name": ex.get("name"), "attempt": attempts, "ok": ok, "err": err})
            if ok or attempts >= REPAIRS:
                break
            attempts += 1
            fixed = json_of(pi(f"This {lang} source failed to render.\n\nSOURCE:\n{src}\n\nRENDERER ERROR:\n{err}\n\nFix it, keeping the same feature being demonstrated ({ex.get('name')}). Reply with ONLY a JSON object, no prose, no fences: {{\"source\":\"...\",\"tip\":\"one sentence on what was wrong\"}}"), "{") or {}
            src, ex["tip"] = fixed.get("source", src), fixed.get("tip", ex.get("tip"))
        if ok:
            kept.append({"name": ex.get("name"), "tip": ex.get("tip"), "source": src, "repairs": attempts})
    firsts = [l for l in log if l["attempt"] == 0]
    return {"drafted": len(draft), "first_try_ok": sum(l["ok"] for l in firsts), "kept": kept, "failures_seen": [l["err"] for l in log if not l["ok"]]}

if __name__ == "__main__":
    out, langs = sys.argv[1], sys.argv[2:]
    res = {}
    for lang in langs:
        t0 = time.time(); res[lang] = run(lang); res[lang]["secs"] = round(time.time() - t0)
        print(lang, f'drafted={res[lang]["drafted"]} first_try_ok={res[lang]["first_try_ok"]} kept={len(res[lang]["kept"])} {res[lang]["secs"]}s', flush=True)
        json.dump(res, open(out, "w"), indent=1)
