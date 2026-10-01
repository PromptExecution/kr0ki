"""Build skills/diagrams/<fmt>/SKILL.md for every skill-src/<fmt>.json, then VERIFY every example by rendering it.
Nothing unverified is written; an example that fails to render is skipped with a message (and the build exits 1).
  skill-src/<fmt>.md    hand-written rules; each bullet must have been confirmed against the renderer
  skill-src/<fmt>.json  {"description": "Use before ...", "identifier": "<identifier rule>", "examples": [{"name","source"}]}
Usage: build_skills.py <out_dir> [fmt ...]   Env: KR0KI_URL (default http://127.0.0.1:8787)"""
import json, os, sys
sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from verify_skills import render

HERE = os.path.dirname(os.path.abspath(__file__))
MAX = 5800  # the agent gate truncates at 6000 chars

def build(fmt, out_dir):
    meta = json.load(open(f"{HERE}/skill-src/{fmt}.json"))
    rules = open(f"{HERE}/skill-src/{fmt}.md").read().strip()
    head = (f"---\nname: kr0ki-{fmt}\ndescription: {meta['description']}\n---\n# {fmt} (render_diagram format `{fmt}`)\n\n"
            f"## Rules (each checked against the renderer)\n{rules}\n\n## Identifiers\n{meta['identifier']}\n\n## Verified examples (all render)\n")
    body, used, skipped = "", 0, 0
    for ex in meta["examples"]:
        ok, err = render(fmt, ex["source"])
        if not ok:
            print(f"  SKIP {fmt}/{ex['name']}: {err[:100]}"); skipped += 1; continue
        block = f"### {ex['name']}\n```{fmt}\n{ex['source'].strip()}\n```\n"
        if len(head) + len(body) + len(block) > MAX:
            print(f"  (size cap: dropped {fmt}/{ex['name']} and later)"); break
        body += block; used += 1
    os.makedirs(f"{out_dir}/{fmt}", exist_ok=True)
    open(f"{out_dir}/{fmt}/SKILL.md", "w").write(head + body)
    print(f"{fmt}: {used} verified examples, {len(head + body)} chars")
    return skipped == 0 and used >= 3

if __name__ == "__main__":
    out = sys.argv[1]
    fmts = sys.argv[2:] or sorted(f[:-5] for f in os.listdir(f"{HERE}/skill-src") if f.endswith(".json"))
    results = [build(f, out) for f in fmts]
    sys.exit(0 if all(results) else 1)
