#!/usr/bin/env python3
"""Packaging tests use synthetic JSON payloads, never generated host artifacts."""
import importlib.util
import json
import os
from pathlib import Path
import shutil
import subprocess
import tarfile
import tempfile
import unittest

spec = importlib.util.spec_from_file_location("bundle", Path(__file__).with_name("docgen-bundle.py"))
bundle = importlib.util.module_from_spec(spec)
spec.loader.exec_module(bundle)


@unittest.skipUnless(shutil.which("zstd"), "zstd is required")
class BundleTests(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory()
        self.addCleanup(self.temp.cleanup)
        self.root = Path(self.temp.name)
        self.source = self.root / "source"
        self.source.mkdir()
        (self.source / "ir.json").write_text(json.dumps({"schema_version": 1, "nodes": []}))

    def package(self, name="output"):
        return bundle.package(self.source, self.root / name, {"revision": "abc", "toolchain": "pinned"})

    def test_repeated_build_ignores_mtime_and_permissions(self):
        first = self.package("first")
        os.utime(self.source / "ir.json", (123456789, 123456789))
        (self.source / "ir.json").chmod(0o755)
        second = self.package("second")
        self.assertEqual(first, second)
        self.assertEqual((self.root / "first/bundle.tar.zst").read_bytes(),
                         (self.root / "second/bundle.tar.zst").read_bytes())

    def test_archive_metadata_and_manifest(self):
        expected = self.package()
        tar = subprocess.run(["zstd", "-dq", "--stdout", str(self.root / "output/bundle.tar.zst")],
                             check=True, stdout=subprocess.PIPE).stdout
        import io
        with tarfile.open(fileobj=io.BytesIO(tar)) as archive:
            self.assertEqual(archive.getnames(), ["bundle-manifest.json", "ir.json"])
            for entry in archive.getmembers():
                self.assertEqual((entry.mtime, entry.uid, entry.gid, entry.mode), (0, 0, 0, 0o644))
            self.assertEqual(json.load(archive.extractfile("bundle-manifest.json")), expected)

    def test_render_does_not_change_semantic_digest(self):
        first = self.package("first")
        (self.source / "diagram.svg").write_text("<svg/>")
        (self.source / "semantic-manifest.json").write_text('{"files":{"diagram.svg":"rendererhash"}}')
        (self.source / "validation.json").write_text('{"render_svg":"passed"}')
        second = self.package("second")
        self.assertEqual(first["semantic_sha256"], second["semantic_sha256"])
        self.assertNotEqual(first["files"], second["files"])

    def test_unknown_payload_is_not_mislabeled_semantic(self):
        (self.source / "ir.json").unlink()
        (self.source / "image.svg").write_text("<svg/>")
        with self.assertRaisesRegex(ValueError, "semantic artifacts"):
            self.package()

    def test_semantic_change_changes_digest(self):
        first = self.package("first")
        (self.source / "ir.json").write_text('{"nodes":["changed"]}')
        self.assertNotEqual(first["semantic_sha256"], self.package("second")["semantic_sha256"])

    def test_rejects_symlinks(self):
        (self.source / "outside").symlink_to("/etc/passwd")
        with self.assertRaisesRegex(ValueError, "symlinks"):
            self.package()

    def test_rejects_nested_output(self):
        with self.assertRaisesRegex(ValueError, "outside"):
            bundle.package(self.source, self.source / "output", {})

    def test_rejects_reserved_manifest(self):
        (self.source / "bundle-manifest.json").write_text("{}")
        with self.assertRaisesRegex(ValueError, "reserved"):
            self.package()


if __name__ == "__main__":
    unittest.main()
