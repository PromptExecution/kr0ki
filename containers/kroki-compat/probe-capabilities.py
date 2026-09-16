#!/usr/bin/env python3
"""Build-time capabilities probe (kr0ki#20).

Boots the real, fully-configured Kroki JVM process this image ships (already
past the PlantUML binary swap — same stage, so the probe result reflects what
actually runs), probes every registered converter with a known-good source
from the vendored kroki.io example catalogue, and writes capabilities.json:
`{"<slug>": {"version": "...", "companion_required": true|false|null,
"note": "..." (present only when uncertain)}}`.

This is the self-reported alternative to kr0ki-core's own `just probe-formats`
(kr0ki#18) empirical discovery tool — same idea (a real HTTP client, buffered
body, checked for a real SVG shape, not just status), computed once at image
build time and baked in, rather than run on demand against a live backend.
`companion_available` is included, and always `false`, only when
`companion_required` is `true` — this image never runs a companion for
anything, by design (PRD NFR3). Matches the issue's own example shape:
`{"symbolator": {"companion_required": false}, "mermaid":
{"companion_required": true, "companion_available": false}}`.

stdlib only (curl and python3 are the only tools this base image has that
aren't the JVM itself — no jq, no pip).
"""

import json
import subprocess
import sys
import time
import urllib.error
import urllib.request

HEALTH_URL = "http://127.0.0.1:8000/health"
EXAMPLES_PATH = "/opt/kr0ki/kroki-examples.json"
OUTPUT_PATH = "/opt/kr0ki/capabilities.json"


def wait_for_health(timeout_s: int = 30) -> None:
    deadline = time.time() + timeout_s
    while time.time() < deadline:
        try:
            with urllib.request.urlopen(HEALTH_URL, timeout=2) as resp:
                if resp.status == 200:
                    return
        except (urllib.error.URLError, OSError):
            pass
        time.sleep(0.5)
    raise SystemExit(f"kroki did not become healthy within {timeout_s}s")


def registered_converters() -> dict:
    with urllib.request.urlopen(HEALTH_URL, timeout=5) as resp:
        body = json.load(resp)
    versions = body.get("version", {})
    return {k: v for k, v in versions.items() if k != "kroki" and isinstance(v, str)}


def probe_sources() -> dict:
    with open(EXAMPLES_PATH, encoding="utf-8") as f:
        fixture = json.load(f)
    sources = {}
    for t in fixture["types"]:
        examples = t.get("examples") or []
        if examples:
            sources[t["type"]] = examples[0]["source"]
    return sources


def probe_one(slug: str, source: str) -> dict:
    url = f"http://127.0.0.1:8000/{slug}/svg"
    req = urllib.request.Request(
        url, data=source.encode("utf-8"), headers={"Content-Type": "text/plain"}, method="POST"
    )
    try:
        with urllib.request.urlopen(req, timeout=15) as resp:
            body = resp.read()
            if body.startswith(b"<?xml") or body.startswith(b"<svg"):
                return {"companion_required": False}
            return {
                "companion_required": None,
                "note": f"200 OK but body doesn't look like SVG ({len(body)} bytes)",
            }
    except urllib.error.HTTPError as e:
        if e.code == 503:
            return {"companion_required": True, "companion_available": False}
        return {"companion_required": None, "note": f"HTTP {e.code}: {e.read()[:200]!r}"}
    except (urllib.error.URLError, OSError) as e:
        return {"companion_required": None, "note": f"request failed: {e}"}


def main() -> None:
    wait_for_health()
    registered = registered_converters()
    sources = probe_sources()

    capabilities = {}
    for slug, version in sorted(registered.items()):
        source = sources.get(slug)
        if source is None:
            capabilities[slug] = {
                "version": version,
                "companion_required": None,
                "note": "no vendored probe source in kroki-examples.json",
            }
            continue
        result = probe_one(slug, source)
        capabilities[slug] = {"version": version, **result}

    with open(OUTPUT_PATH, "w", encoding="utf-8") as f:
        json.dump(capabilities, f, indent=2, sort_keys=True)
        f.write("\n")

    for slug, info in sorted(capabilities.items()):
        print(f"  {slug:<16} {info}", file=sys.stderr)
    print(f"wrote {len(capabilities)} entries to {OUTPUT_PATH}", file=sys.stderr)


if __name__ == "__main__":
    main()
