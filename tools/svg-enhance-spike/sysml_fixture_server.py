"""Tiny read-only stand-in for an OMG SysML v2 API server: one project, one commit, a small vehicle model.
Real-looking element ids (UUIDs) and qualified names, so we can test how identifiers survive rendering.
Usage: python3 sysml_fixture_server.py [port]   (default 18081)"""
import json, sys
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer

U = lambda n: f"00000000-0000-4000-8000-{n:012d}"
ELEMENTS = [
    {"@id": U(1), "@type": "PartDefinition", "name": "Vehicle", "qualifiedName": "Demo::Vehicle", "declaredName": "Vehicle"},
    {"@id": U(2), "@type": "PartUsage", "name": "engine", "qualifiedName": "Demo::Vehicle::engine"},
    {"@id": U(3), "@type": "PartUsage", "name": "transmission", "qualifiedName": "Demo::Vehicle::transmission"},
    {"@id": U(4), "@type": "PartUsage", "name": "battery", "qualifiedName": "Demo::Vehicle::battery"},
    {"@id": U(5), "@type": "RequirementUsage", "name": "range", "qualifiedName": "Demo::Vehicle::range"},
    {"@id": U(10), "@type": "FeatureMembership", "owner": {"@id": U(1)}, "member": {"@id": U(2)}},
    {"@id": U(11), "@type": "FeatureMembership", "owner": {"@id": U(1)}, "member": {"@id": U(3)}},
    {"@id": U(12), "@type": "FeatureMembership", "owner": {"@id": U(1)}, "member": {"@id": U(4)}},
    {"@id": U(13), "@type": "SatisfyRequirementUsage", "source": {"@id": U(4)}, "target": {"@id": U(5)}},
]
COMMIT = {"@id": "c1", "@type": "Commit", "created": "2026-10-01T00:00:00Z"}

class H(BaseHTTPRequestHandler):
    def log_message(self, *a): pass
    def send(self, code, body):
        data = json.dumps(body).encode()
        self.send_response(code); self.send_header("Content-Type", "application/json"); self.send_header("Content-Length", str(len(data))); self.end_headers(); self.wfile.write(data)
    def do_GET(self):
        p = self.path.split("?")[0].strip("/").split("/")
        if p == ["projects"]: return self.send(200, [{"@id": "p1", "name": "demo", "defaultBranch": {"@id": "b1"}}])
        if p == ["projects", "p1", "commits"]: return self.send(200, [COMMIT])
        if p == ["projects", "p1", "commits", "c1", "elements"]: return self.send(200, ELEMENTS)
        if p == ["projects", "p1", "commits", "c1", "roots"]: return self.send(200, [U(1)])
        self.send(404, {"error": "not found", "path": self.path})

if __name__ == "__main__":
    ThreadingHTTPServer(("127.0.0.1", int(sys.argv[1]) if len(sys.argv) > 1 else 18081), H).serve_forever()
