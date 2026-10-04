#!/usr/bin/env python3
"""Package validated docgen output without inheriting host filesystem metadata."""

import argparse
import hashlib
import io
import json
from pathlib import Path
import subprocess
import tarfile


def canonical_json(value):
    return (json.dumps(value, sort_keys=True, separators=(",", ":"), ensure_ascii=False) + "\n").encode()


def digest(data):
    return hashlib.sha256(data).hexdigest()


def collect_files(source):
    files = {}
    for path in sorted(source.rglob("*")):
        if path.is_symlink():
            raise ValueError(f"symlinks are not artifact inputs: {path.relative_to(source)}")
        if path.is_dir():
            continue
        if not path.is_file():
            raise ValueError(f"non-regular artifact input: {path.relative_to(source)}")
        files[path.relative_to(source).as_posix()] = path.read_bytes()
    if not files:
        raise ValueError("artifact directory is empty")
    return files


def archive_bytes(files):
    result = io.BytesIO()
    with tarfile.open(fileobj=result, mode="w", format=tarfile.PAX_FORMAT) as archive:
        for name, data in sorted(files.items()):
            entry = tarfile.TarInfo(name)
            entry.size = len(data)
            entry.mode = 0o644
            entry.uid = entry.gid = entry.mtime = 0
            entry.uname = entry.gname = ""
            archive.addfile(entry, io.BytesIO(data))
    return result.getvalue()


def package(source, output, metadata):
    source, output = source.resolve(), output.resolve()
    if output == source or source in output.parents:
        raise ValueError("bundle output must be outside the artifact input directory")
    files = collect_files(source)
    hashes = {name: digest(data) for name, data in files.items()}
    semantic_json = {"ir.json", "schema.json", "graph.json", "view.json", "lint.json"}
    semantic_notation = {".sysml", ".d2", ".mmd", ".scxml"}
    semantic_hashes = {name: value for name, value in hashes.items()
                       if name in semantic_json or Path(name).suffix in semantic_notation}
    if not semantic_hashes:
        raise ValueError("artifact directory has no recognized semantic artifacts")
    manifest = {"schema_version": 1, "inputs": metadata, "files": hashes, "semantic_files": semantic_hashes,
                "semantic_sha256": digest(canonical_json(semantic_hashes))}
    manifest_bytes = canonical_json(manifest)
    if "bundle-manifest.json" in files:
        raise ValueError("bundle-manifest.json is reserved for the packaging manifest")
    files["bundle-manifest.json"] = manifest_bytes
    compressed = subprocess.run(
        ["zstd", "-q", "-19", "-T1", "--no-progress", "--stdout"],
        input=archive_bytes(files), check=True, stdout=subprocess.PIPE).stdout
    output.mkdir(parents=True, exist_ok=True)
    (output / "bundle.tar.zst").write_bytes(compressed)
    (output / "manifest.json").write_bytes(manifest_bytes)
    (output / "bundle.sha256").write_text(digest(compressed) + "  bundle.tar.zst\n")
    return manifest


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--input", type=Path, required=True)
    parser.add_argument("--output", type=Path, required=True)
    parser.add_argument("--metadata", type=Path, required=True)
    args = parser.parse_args()
    metadata = json.loads(args.metadata.read_text())
    if not isinstance(metadata, dict):
        parser.error("metadata must be a JSON object")
    manifest = package(args.input, args.output, metadata)
    print(json.dumps({"semantic_sha256": manifest["semantic_sha256"]}, sort_keys=True))


if __name__ == "__main__":
    main()
