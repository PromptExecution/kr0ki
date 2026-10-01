"""Check the per-type skills (skills/types/<id>.md): every fenced example must render, and each file must have the
required sections. Usage: verify_type_skills.py <skills/types dir> [id ...]   Env: KR0KI_URL.
A fence's info string is the render format (```plantuml); ```k8s-topology is posted to /render/k8s-topology."""
import os, re, sys, urllib.error, urllib.request, json
KR0KI = os.environ.get("KR0KI_URL", "http://127.0.0.1:8787")
SECTIONS = ["## Choose it when", "## Not when", "## Anatomy", "## What makes it good", "## What makes it bad", "## Questions to ask", "## Contrast"]
MAX = 4500

def render(fmt, src):
    route = "/render/k8s-topology" if fmt == "k8s-topology" else f"/render/{fmt}"
    req = urllib.request.Request(f"{KR0KI}{route}?output=svg", data=src.encode(), headers={"Content-Type": "text/plain"})
    try:
        urllib.request.urlopen(req, timeout=60); return True, ""
    except urllib.error.HTTPError as e:
        body = e.read().decode("utf-8", "replace")
        try: body = json.loads(body).get("message", body)
        except ValueError: pass
        return False, " ".join(body.split())[:200]
    except Exception as e:
        return False, str(e)[:200]

if __name__ == "__main__":
    root = sys.argv[1]; ids = sys.argv[2:] or sorted(f[:-3] for f in os.listdir(root) if f.endswith(".md"))
    bad = 0
    for tid in ids:
        text = open(f"{root}/{tid}.md").read()
        probs = [f"missing '{s}'" for s in SECTIONS if s not in text]
        if len(text) > MAX: probs.append(f"{len(text)} chars > {MAX}")
        fences = re.findall(r"```([a-z0-9-]+)\n(.*?)\n```", text, re.S)
        if len(fences) < 2: probs.append("needs at least two fenced examples (bad, then good)")
        for fmt, src in fences:
            ok, err = render(fmt, src)
            if not ok: probs.append(f"{fmt} example does not render: {err[:100]}")
        if probs:
            bad += 1; print(f"FAIL {tid}: " + "; ".join(probs))
    print(f"{len(ids) - bad}/{len(ids)} type skills ok"); sys.exit(1 if bad else 0)
