"""Build skills/diagrams/<fmt>/SKILL.md from hand-written rules (skill-src/<fmt>.md) and examples, then VERIFY every
example by rendering it. Nothing unverified is written. Usage: build_skills.py <corpus.json> <out_dir>
Env: KR0KI_URL (default http://127.0.0.1:8787). Corpus: generate.py output; nwdiag examples are hand-written below."""
import json, os, sys, textwrap
sys.path.insert(0, os.path.dirname(__file__))
from verify_skills import render

HERE = os.path.dirname(os.path.abspath(__file__))
MAX = 5800
IDENT = {
    "d2": "When an identifier exists (a SysML v2 element id, `schema.table`, `namespace/kind/name`), use it as the node **key** and put the display name in the **label**: `\"00000000-0000-4000-8000-000000000002\": engine`. Never invent identifiers.",
    "graphviz": "When an identifier exists, use it as the node id and the display name as the label: `\"00000000-...02\" [label=\"engine\"];`. Never invent identifiers.",
    "plantuml": "When an identifier exists, keep it as the element alias and the display name as its title: `class \"Engine\" as e0002`. Never invent identifiers.",
    "nwdiag": "When an identifier exists, use it as the node name and the display name as `label`: `n0002 [label = \"engine\"];`. Never invent identifiers.",
}
DESC = {
    "d2": "Use before writing or fixing D2 source for render_diagram (format d2): verified syntax rules, quoting/escaping gotchas, worked examples, identifier rule.",
    "graphviz": "Use before writing or fixing Graphviz DOT source for render_diagram (format graphviz): verified syntax rules, record/port escaping, clusters, worked examples, identifier rule.",
    "plantuml": "Use before writing or fixing PlantUML source (class diagrams especially) for render_diagram (format plantuml): the @startuml wrapper, relationship syntax, worked examples, identifier rule.",
    "nwdiag": "Use before writing or fixing nwdiag network diagrams for render_diagram (format nwdiag): network/node syntax, valid attributes and shapes, worked examples, identifier rule.",
}
NWDIAG = [
 ("two networks sharing nodes", 'nwdiag {\n  network dmz {\n    address = "210.x.x.x/24"\n    web01 [address = "210.x.x.1"];\n    web02 [address = "210.x.x.2"];\n  }\n  network internal {\n    address = "172.x.x.x/24";\n    web01 [address = "172.x.x.1"];\n    web02 [address = "172.x.x.2"];\n    db01;\n    app01 [address = "172.x.x.100, 172.x.x.101"];\n  }\n}'),
 ("shapes and multi-line labels", 'nwdiag {\n  network office {\n    router01 [shape = "cloud", label = "Edge\\nrouter"];\n    pc01 [shape = "actor"];\n    printer [shape = "note", color = "#DDEEFF"];\n  }\n}'),
 ("network colour and label", 'nwdiag {\n  network lan {\n    address = "192.168.0.0/24";\n    color = "#CCFFCC";\n    label = "Office LAN";\n    nas [address = "192.168.0.10"];\n    laptop [address = "192.168.0.20"];\n  }\n}'),
 ("peer link to the internet", 'nwdiag {\n  inet [shape = "cloud"];\n  inet -- router;\n  network lan {\n    router;\n    host1;\n  }\n}'),
 ("quoted names with spaces", 'nwdiag {\n  network "front end" {\n    "web server 1" [address = "10.0.0.1"];\n  }\n}'),
]

def build(fmt, examples, out_dir):
    rules = open(f"{HERE}/skill-src/{fmt}.md").read().strip()
    head = f"---\nname: kr0ki-{fmt}\ndescription: {DESC[fmt]}\n---\n# {fmt} (render_diagram format `{fmt}`)\n\n## Rules (each checked against the renderer, 2026-10-01)\n{rules}\n\n## Identifiers\n{IDENT[fmt]}\n\n## Verified examples (all render)\n"
    body, used = "", 0
    for name, src in examples:
        ok, err = render(fmt, src)
        if not ok:
            print(f"  skip {fmt}/{name}: {err[:90]}"); continue
        block = f"### {name}\n```{fmt}\n{src.strip()}\n```\n"
        if len(head) + len(body) + len(block) > MAX:
            break
        body += block; used += 1
    os.makedirs(f"{out_dir}/{fmt}", exist_ok=True)
    open(f"{out_dir}/{fmt}/SKILL.md", "w").write(head + body)
    print(f"{fmt}: {used} verified examples, {len(head + body)} chars")

if __name__ == "__main__":
    corpus = json.load(open(sys.argv[1])); out = sys.argv[2]
    for fmt in ("d2", "graphviz", "plantuml"):
        build(fmt, [(k["name"], k["source"]) for k in corpus[fmt]["kept"]], out)
    build("nwdiag", NWDIAG, out)
