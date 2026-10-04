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
import hashlib, io, json, os, pathlib, subprocess, sys, tarfile
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
    if '--format' in args:
        print('sha256:fixture-image-id')
    else:
        records = [json.loads(line) for line in pathlib.Path(os.environ['DOCGEN_TEST_LOG']).read_text().splitlines()]
        build = next(record['args'] for record in reversed(records) if record['args'][0] == 'build' and args[2] in record['args'])
        labels = dict(build[i + 1].split('=', 1) for i, arg in enumerate(build) if arg == '--label')
        if os.environ.get('DOCGEN_TEST_STALE_LABEL'):
            labels[os.environ['DOCGEN_TEST_STALE_LABEL']] = 'stale-cached-value'
        print(json.dumps([{'Id': 'sha256:fixture-image-id', 'Labels': labels}]))
if args and args[0] == 'run' and 'kr0ki-docgen-renderer:' in args[-1]:
    mount = args[args.index('-v') + 1].split(':/export:')[0]
    output = pathlib.Path(mount)
    payload = b'fixture rendered bundle'
    (output / 'bundle.tar.zst').write_bytes(payload)
    (output / 'bundle.sha256').write_text(hashlib.sha256(payload).hexdigest() + '  bundle.tar.zst\\n')
    (output / 'manifest.json').write_text('{}')
if args and args[0] == 'create':
    print('fixture-container')
if args and args[0] == 'cp':
    output = pathlib.Path(args[-1])
    records = [json.loads(line) for line in pathlib.Path(os.environ['DOCGEN_TEST_LOG']).read_text().splitlines()]
    inputs = next(record['inputs'] for record in reversed(records) if record['args'][0] == 'build')
    canonical = lambda value: (json.dumps(value, sort_keys=True, separators=(',', ':')) + '\\n').encode()
    files = {'ir.json': b'{}\\n'}
    hashes = {name: hashlib.sha256(data).hexdigest() for name, data in files.items()}
    manifest = {'schema_version': 1, 'inputs': inputs, 'files': hashes, 'semantic_files': hashes,
                'semantic_sha256': hashlib.sha256(canonical(hashes)).hexdigest()}
    manifest_bytes = canonical(manifest)
    files['bundle-manifest.json'] = manifest_bytes
    if os.environ.get('DOCGEN_TEST_BAD_ARTIFACT'):
        files['ir.json'] = b'corrupt fixture'
    stream = io.BytesIO()
    with tarfile.open(fileobj=stream, mode='w') as archive:
        for name, data in sorted(files.items()):
            member = tarfile.TarInfo(name)
            member.size, member.mode = len(data), 0o644
            archive.addfile(member, io.BytesIO(data))
    payload = subprocess.run(['zstd', '-q', '--stdout'], input=stream.getvalue(),
                             check=True, stdout=subprocess.PIPE).stdout
    (output / 'bundle.tar.zst').write_bytes(payload)
    (output / 'bundle.sha256').write_text(hashlib.sha256(payload).hexdigest() + '  bundle.tar.zst\\n')
    (output / 'manifest.json').write_bytes(manifest_bytes)
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
        labels = [record["args"][i + 1] for i, arg in enumerate(record["args"]) if arg == "--label"]
        self.assertIn("org.opencontainers.image.revision=" + record["inputs"]["revision"], labels)
        self.assertIn("org.kr0ki.docgen.source-tree=" + record["inputs"]["source_tree"], labels)

    def test_self_test_has_explicit_container_isolation(self):
        result = self.run_workflow("self-test")
        self.assertEqual(result.returncode, 0, result.stderr)
        args = next(record["args"] for record in self.records() if record["args"][0] == "run")
        for required in ["--read-only", "--cap-drop=ALL", "--security-opt=no-new-privileges",
                         "--network=none", "--memory=8g", "--cpus=1", "--pids-limit=512"]:
            self.assertIn(required, args)
        self.assertFalse(any(arg.startswith(("--privileged", "--volume", "-v")) for arg in args))

    def test_rejects_manifest_outside_tracked_source(self):
        result = self.run_workflow("image", KR0KI_DOCGEN_MANIFEST="../host/Cargo.toml")
        self.assertNotEqual(result.returncode, 0)
        self.assertIn("relative", result.stderr)
        self.assertFalse(self.log.exists())

    def test_rejects_stale_cached_image_labels_before_use(self):
        for key in ["org.opencontainers.image.revision", "org.kr0ki.docgen.source-tree",
                    "org.kr0ki.docgen.schema", "org.kr0ki.docgen.rules", "org.kr0ki.docgen.configuration"]:
            with self.subTest(label=key):
                result = self.run_workflow("self-test", DOCGEN_TEST_STALE_LABEL=key)
                self.assertNotEqual(result.returncode, 0)
                self.assertIn("OCI metadata mismatch: " + key, result.stderr)
                self.assertFalse(any(record["args"][0] in {"run", "create", "cp"} for record in self.records()))

    def test_export_independently_verifies_artifact_digests(self):
        result = self.run_workflow("artifacts")
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertIn("Verified image metadata", result.stdout)
        result = self.run_workflow("artifacts", DOCGEN_TEST_BAD_ARTIFACT="1")
        self.assertNotEqual(result.returncode, 0)
        self.assertIn("artifact digests do not match", result.stderr)

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
