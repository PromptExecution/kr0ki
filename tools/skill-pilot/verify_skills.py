"""Acceptance check (WP1): every fenced example in skills/diagrams/*/SKILL.md must render against kr0ki.
Usage: verify_skills.py <skills/diagrams dir>   Exit 1 on any failure. Env: KR0KI_URL (default http://127.0.0.1:8787)."""
import json, os, re, sys, urllib.error, urllib.request
KR0KI = os.environ.get("KR0KI_URL", "http://127.0.0.1:8787")

def render(fmt, src):
    req = urllib.request.Request(f"{KR0KI}/render/{fmt}?output=svg", data=src.encode(), headers={"Content-Type": "text/plain"})
    try:
        urllib.request.urlopen(req, timeout=60); return True, ""
    except urllib.error.HTTPError as e:
        body = e.read().decode("utf-8", "replace")
        try: body = json.loads(body).get("message", body)
        except ValueError: pass
        return False, " ".join(body.split())[:300]
    except Exception as e:
        return False, str(e)[:200]

if __name__ == "__main__":
    root, bad, total = sys.argv[1], 0, 0
    for fmt in sorted(os.listdir(root)):
        text = open(f"{root}/{fmt}/SKILL.md").read()
        for m in re.finditer(r"```" + re.escape(fmt) + r"\n(.*?)\n```", text, re.S):
            total += 1
            ok, err = render(fmt, m.group(1))
            if not ok:
                bad += 1; print(f"FAIL {fmt}: {err[:120]}")
    print(f"{total - bad}/{total} examples render"); sys.exit(1 if bad else 0)
