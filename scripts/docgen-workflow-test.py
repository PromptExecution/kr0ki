#!/usr/bin/env python3
"""Exercise tracked-source isolation and container policy without a Podman daemon."""
import json
import os
from pathlib import Path
import shutil
import subprocess
import tempfile
import unittest


class WorkflowTests(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory()
        self.addCleanup(self.temp.cleanup)
        self.root = Path(self.temp.name)
        self.repo = self.root / "repo"
        self.repo.mkdir()
        self.bin = self.root / "bin"
        self.bin.mkdir()
        self.log = self.root / "podman.jsonl"
        script = Path(__file__).with_name("docgen.sh")
        paths = ["scripts/docgen.sh", "Cargo.lock", "tools/rust-behavior-extractor/Cargo.lock",
                 "tools/rust-behavior-extractor/tests/fixtures/workspace/Cargo.toml",
                 "crates/kr0ki-behavior/src/lib.rs", "crates/kr0ki-behavior/src/schema.rs",
                 "crates/kr0ki-core/src/rust_behavior.rs", "scripts/docgen-bundle.py",
                 "containers/kr0ki-docgen/Containerfile"]
        for relative in paths:
            path = self.repo / relative
            path.parent.mkdir(parents=True, exist_ok=True)
            if relative == "scripts/docgen.sh":
                shutil.copy2(script, path)
            else:
                path.write_text("tracked fixture\n")
        self.git("init", "-q")
        self.git("config", "user.email", "fixture@example.invalid")
        self.git("config", "user.name", "Docgen fixture")
        self.git("add", ".")
        self.git("commit", "-q", "--no-gpg-sign", "-m", "fixture")
        (self.repo / ".env").write_text("SECRET=must-not-enter-context\n")
        (self.repo / "crates/kr0ki-behavior/src/lib.rs").write_text("uncommitted host edit\n")
        mock = self.bin / "podman"
        mock.write_text("""#!/usr/bin/env python3
import hashlib, json, os, pathlib, sys
args = sys.argv[1:]
record = {'args': args}
if args and args[0] == 'build':
    context = pathlib.Path(args[-1])
    record['files'] = sorted(p.relative_to(context / 'source').as_posix() for p in (context / 'source').rglob('*') if p.is_file())
    record['lib'] = (context / 'source/crates/kr0ki-behavior/src/lib.rs').read_text()
    record['inputs'] = json.loads((context / 'build-inputs.json').read_text())
    record['rendered_files'] = sorted(p.name for p in (context / 'rendered').glob('*'))
with open(os.environ['DOCGEN_TEST_LOG'], 'a') as log:
    log.write(json.dumps(record) + '\\n')
if args[:2] == ['image', 'inspect']:
    print('sha256:fixture-image-id')
if args and args[0] == 'run' and 'kr0ki-docgen-renderer:' in args[-1]:
    mount = args[args.index('-v') + 1].split(':/export:')[0]
    output = pathlib.Path(mount)
    payload = b'fixture rendered bundle'
    (output / 'bundle.tar.zst').write_bytes(payload)
    (output / 'bundle.sha256').write_text(hashlib.sha256(payload).hexdigest() + '  bundle.tar.zst\\n')
    (output / 'manifest.json').write_text('{}')
""")
        mock.chmod(0o755)
        self.env = dict(os.environ, PATH=str(self.bin) + os.pathsep + os.environ["PATH"],
                        DOCGEN_TEST_LOG=str(self.log))
        for key in list(self.env):
            if key.startswith("KR0KI_DOCGEN_"):
                self.env.pop(key)

    def git(self, *args):
        subprocess.run(["git", "-C", str(self.repo), *args], check=True,
                       stdout=subprocess.PIPE, stderr=subprocess.PIPE)

    def run_workflow(self, operation, **configuration):
        return subprocess.run(["bash", str(self.repo / "scripts/docgen.sh"), operation, "HEAD"],
                              env=dict(self.env, **configuration),
                              stdout=subprocess.PIPE, stderr=subprocess.PIPE, text=True)

    def records(self):
        return [json.loads(line) for line in self.log.read_text().splitlines()]

    def test_build_uses_committed_tree_without_env_or_working_edits(self):
        result = self.run_workflow("image")
        self.assertEqual(result.returncode, 0, result.stderr)
        record = self.records()[0]
        self.assertNotIn(".env", record["files"])
        self.assertEqual(record["lib"], "tracked fixture\n")
        self.assertIn("@sha256:", record["inputs"]["builder_image"])
        self.assertEqual(len(record["inputs"]["source_tree"]), 64)
        self.assertEqual(len(record["inputs"]["git_tree_id"]), 40)
        self.assertIn("--timestamp=0", record["args"])

    def test_self_test_has_explicit_container_isolation(self):
        result = self.run_workflow("self-test")
        self.assertEqual(result.returncode, 0, result.stderr)
        args = self.records()[-1]["args"]
        for required in ["--read-only", "--cap-drop=ALL", "--security-opt=no-new-privileges",
                         "--network=none", "--memory=8g", "--cpus=1", "--pids-limit=512"]:
            self.assertIn(required, args)
        self.assertFalse(any(arg.startswith(("--privileged", "--volume", "-v")) for arg in args))

    def test_rejects_manifest_outside_tracked_source(self):
        result = self.run_workflow("image", KR0KI_DOCGEN_MANIFEST="../host/Cargo.toml")
        self.assertNotEqual(result.returncode, 0)
        self.assertIn("relative", result.stderr)
        self.assertFalse(self.log.exists())

    def test_backend_requires_explicit_network(self):
        result = self.run_workflow("self-test", KR0KI_DOCGEN_BACKEND="http://private.invalid:8010")
        self.assertNotEqual(result.returncode, 0)
        self.assertIn("explicit", result.stderr)
        self.assertFalse(self.log.exists())

    def test_backend_renders_evidence_after_offline_compilation(self):
        result = self.run_workflow("self-test", KR0KI_DOCGEN_BACKEND="http://private.invalid:8010",
                                   KR0KI_DOCGEN_NETWORK="private-test")
        self.assertEqual(result.returncode, 0, result.stderr)
        runs = [record["args"] for record in self.records() if record["args"][0] == "run"]
        self.assertEqual(len(runs), 2)
        self.assertIn("--network=none", runs[0])
        self.assertFalse(any("KR0KI_DOCGEN_BACKEND=" in arg for arg in runs[0]))
        self.assertIn("--network=private-test", runs[1])
        self.assertIn("kr0ki-docgen-renderer:", runs[1][-1])
        self.assertFalse(any(arg.startswith("--userns=keep-id") for arg in runs[1]))

    def test_rendered_artifact_is_assembled_offline(self):
        result = self.run_workflow("image", KR0KI_DOCGEN_BACKEND="http://user:secret@private.invalid:8010",
                                   KR0KI_DOCGEN_NETWORK="private-test")
        self.assertEqual(result.returncode, 0, result.stderr)
        builds = [record for record in self.records() if record["args"][0] == "build"]
        self.assertEqual(builds[-1]["args"][builds[-1]["args"].index("--target") + 1], "rendered-artifact")
        self.assertIn("--network=none", builds[-1]["args"])
        self.assertIn("bundle.tar.zst", builds[-1]["rendered_files"])
        self.assertEqual(builds[-1]["inputs"]["configuration"]["backend"], "http://private.invalid:8010")


if __name__ == "__main__":
    unittest.main()
