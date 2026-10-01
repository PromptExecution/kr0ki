"""A/B: does a language skill help? Same drawing tasks, local model via pi, with vs without the SKILL.md body in the
prompt; the renderer judges. Usage: ab.py <skills/diagrams dir> <out.json>   Env: PI_SMOKE_DIR, KR0KI_URL.
Single sample per cell and a non-deterministic model: treat differences as indicative, not statistical proof."""
import json, os, re, subprocess, sys
sys.path.insert(0, os.path.dirname(__file__))
from verify_skills import render

PI = os.environ["PI_SMOKE_DIR"]
mid = json.load(open(f"{PI}/agent/models.json"))["providers"]["local"]["models"][0]["id"]
env = {**os.environ, "PI_CODING_AGENT_DIR": f"{PI}/agent", "PI_OFFLINE": "1", "PI_SKIP_VERSION_CHECK": "1", "PI_TELEMETRY": "0"}
TASKS = {
 "d2": ["a three-tier web app: browser, load balancer, two app servers, a Postgres database drawn as a cylinder", "an order workflow with a decision node and labelled arrows, blue fill on the start node", "a service named 'Billing ($)' that calls 'Auth' and 'Ledger', grouped inside a container called platform", "a database table 'users' with id, email and created_at columns", "a sequence of messages between client, api and cache", "three boxes in a row with a dotted arrow from the first to the last and a markdown note"],
 "graphviz": ["a build pipeline: source, build, test, deploy as left-to-right boxes", "a record-shaped node listing name|age|address fields and an edge to a second record", "two clusters, frontend and backend, with edges between their nodes", "a decision flow with diamonds and yes/no edge labels", "services whose names contain spaces: 'Auth Service' calls 'User DB'", "same-rank nodes a, b, c under a common parent"],
 "plantuml": ["a class diagram: Animal with subclasses Dog and Cat, Dog has a list of Toy", "an interface Repository<T> implemented by UserRepository, with private and public members", "a package 'billing' containing Invoice and Payment classes, Invoice aggregates Payment", "an abstract class Shape with Circle and Square, plus a note on Shape", "an enum Status with three values used by an Order class", "a class diagram with a stereotype <<Entity>> and hidden empty members"],
 "nwdiag": ["a DMZ with two web servers and an internal network with a database, web servers on both", "an office LAN 192.168.0.0/24 with a NAS, a laptop and a printer", "an internet cloud connected to a router that sits on a LAN with two hosts", "a front-end network and a back-end network joined by a gateway node with two addresses", "a labelled, coloured management network with three switches", "a database server on an internal network and an app server on both internal and public networks"],
}
def pi(prompt):
    r = subprocess.run(["pi", "--offline", "--no-session", "--provider", "local", "--model", mid, "--no-tools", "--no-extensions",
                        "--no-skills", "--no-context-files", "-p", prompt], cwd=f"{PI}/work", env=env, stdin=subprocess.DEVNULL, capture_output=True, text=True, timeout=420)
    return r.stdout
def strip_fences(t):
    m = re.search(r"```[a-zA-Z0-9]*\n(.*?)```", t, re.S)
    return (m.group(1) if m else t).strip()
def guide(root, fmt):
    t = open(f"{root}/{fmt}/SKILL.md").read()
    return t.split("---", 2)[2].strip()
if __name__ == "__main__":
    root, out = sys.argv[1], sys.argv[2]
    res = {}
    only = os.environ.get('AB_ONLY')
    for fmt, tasks in TASKS.items():
        if only and fmt not in only.split(','): continue
        g = guide(root, fmt)
        res[fmt] = {"plain": [], "skill": []}
        for t in tasks:
            base = f"Write {fmt} diagram source (rendered by Kroki) for: {t}. Reply with ONLY the {fmt} source, nothing else."
            for arm, prompt in (("plain", base), ("skill", f"Syntax guide for {fmt} (follow it):\n\n{g}\n\n---\n{base}")):
                src = strip_fences(pi(prompt)); ok, err = render(fmt, src)
                res[fmt][arm].append({"task": t, "ok": ok, "err": err[:160]})
        p = sum(x["ok"] for x in res[fmt]["plain"]); s = sum(x["ok"] for x in res[fmt]["skill"])
        print(f"{fmt}: plain {p}/{len(tasks)}  with skill {s}/{len(tasks)}", flush=True)
        json.dump(res, open(out, "w"), indent=1)
