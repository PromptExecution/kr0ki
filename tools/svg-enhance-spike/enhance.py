"""Spike: an SVG enhancement layer over a rendered diagram.

Contract (the thing worth keeping; the Python is throwaway):
  1. INDEX  - find each diagram element by its well-known identifier and STAMP it with stable attributes
              (data-kr0ki-id, data-kr0ki-type, data-kr0ki-name). D2 hides identifiers as base64 class names
              on <g>; Graphviz exposes them in <title>. After stamping, nothing downstream needs renderer knowledge.
  2. RULES  - a brand/theme config (brand.example.json) rewrites the stamped DOM: add classes, inject icons,
              restore display names. Selection after stamping is plain CSS (`[data-kr0ki-type="PartUsage"]`),
              so the same rules run in the browser (querySelectorAll) or on the server (Rust `scraper`/`lol_html`).
The diagram source stays the version-controlled base layer; this layer is replayable and replaceable.
"""
import base64, copy, json, re, sys
import xml.etree.ElementTree as ET

SVG = "http://www.w3.org/2000/svg"
ET.register_namespace("", SVG)
ET.register_namespace("xlink", "http://www.w3.org/1999/xlink")
Q = lambda t: f"{{{SVG}}}{t}"


def b64_class(identifier: str) -> str:
    """The class D2 puts on an element whose key is `identifier` (unpadded base64 of the key)."""
    return base64.b64encode(identifier.encode()).decode().rstrip("=")


def index(root: ET.Element, known: dict) -> dict:
    """Stamp every <g> whose class encodes a known identifier. `known`: id -> {"type":..., "name":...}.
    Returns id -> element. Idempotent; unknown identifiers are left untouched."""
    wanted = {b64_class(i): i for i in known}
    found = {}
    for g in root.iter(Q("g")):
        for cls in (g.get("class") or "").split():
            ident = wanted.get(cls)
            if ident is not None:
                meta = known[ident]
                g.set("data-kr0ki-id", ident)
                if meta.get("type"): g.set("data-kr0ki-type", meta["type"])
                if meta.get("name"): g.set("data-kr0ki-name", meta["name"])
                found[ident] = g
    return found


def _select(root, selector: str):
    """The tiny CSS subset the spike supports: [data-kr0ki-type="X"] and [data-kr0ki-id="X"]. (Browser: use real CSS.)"""
    m = re.fullmatch(r'\[(data-kr0ki-(?:type|id|name))="([^"]+)"\]', selector.strip())
    if not m:
        raise ValueError(f"unsupported selector in spike: {selector}")
    attr, val = m.groups()
    return [g for g in root.iter(Q("g")) if g.get(attr) == val]


def apply_brand(root: ET.Element, brand: dict) -> int:
    """Apply brand rules; returns number of element rewrites. Safe to run twice."""
    defs = root.find(Q("defs"))
    if defs is None:
        defs = ET.Element(Q("defs")); root.insert(0, defs)
    for name, icon in brand.get("icons", {}).items():
        if not any(s.get("id") == f"kr0ki-icon-{name}" for s in defs):
            sym = ET.SubElement(defs, Q("symbol"), {"id": f"kr0ki-icon-{name}", "viewBox": icon.get("viewBox", "0 0 24 24")})
            sym.append(ET.fromstring(f'<g xmlns="{SVG}">{icon["svg"]}</g>'))
    n = 0
    for rule in brand.get("rules", []):
        for g in _select(root, rule["select"]):
            if "class" in rule and rule["class"] not in (g.get("class") or "").split():
                g.set("class", f'{g.get("class", "")} {rule["class"]}'.strip()); n += 1
            if "icon" in rule and not any(c.tag == Q("use") and c.get("data-kr0ki-icon") for c in g):
                box = next((c for c in g.iter(Q("rect"))), None)
                if box is not None:
                    x, y = float(box.get("x", 0)), float(box.get("y", 0))
                    ET.SubElement(g, Q("use"), {"href": f'#kr0ki-icon-{rule["icon"]}', "data-kr0ki-icon": rule["icon"],
                                                "x": str(x + 6), "y": str(y + 6), "width": "20", "height": "20"})
                    n += 1
            if rule.get("label") == "name" and g.get("data-kr0ki-name"):
                for t in g.iter(Q("text")):
                    if t.text == g.get("data-kr0ki-id"):
                        t.text = g.get("data-kr0ki-name"); n += 1
    css = brand.get("css")
    if css and not any(c.tag == Q("style") and c.get("id") == "kr0ki-brand" for c in root):
        st = ET.SubElement(root, Q("style"), {"id": "kr0ki-brand"}); st.text = css
    return n


def enhance(svg_text: str, known: dict, brand: dict):
    root = ET.fromstring(svg_text)
    found = index(root, known)
    n = apply_brand(root, brand)
    return ET.tostring(root, encoding="unicode"), {"indexed": len(found), "unindexed": sorted(set(known) - set(found)), "rewrites": n}


if __name__ == "__main__":
    svg, snapshot, brand_path = sys.argv[1:4]
    elements = json.load(open(snapshot))
    known = {e["@id"]: {"type": e["@type"], "name": e.get("name")} for e in elements if e.get("name")}
    out, report = enhance(open(svg).read(), known, json.load(open(brand_path)))
    open(sys.argv[4] if len(sys.argv) > 4 else "enhanced.svg", "w").write(out)
    print(json.dumps(report))
