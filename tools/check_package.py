"""Check an OHOS release ZIP without extracting it. Not a device runtime test."""
import argparse
import hashlib
import json
from pathlib import PurePosixPath
import struct
import zipfile


def validate(path):
    with zipfile.ZipFile(path) as archive:
        files = [name for name in archive.namelist() if not name.endswith("/")]
        if len(files) != len(set(files)):
            raise ValueError("Duplicate ZIP entries")
        if any(PurePosixPath(name).is_absolute() or ".." in PurePosixPath(name).parts or
               "\\" in name for name in files):
            raise ValueError("Unsafe ZIP path")
        roots = {name.split("/")[0] for name in files}
        if len(roots) != 1:
            raise ValueError("Expected one SDK root directory")
        prefix = roots.pop() + "/"
        entries = {name[len(prefix):]: archive.read(name) for name in files}
    required = {"SHA256SUMS", "BUILD-MANIFEST.json", "include/tantivy_ohos.h",
                "lib/arm64-v8a/libtantivy_ohos.so", "LICENSE", "NOTICE",
                "source/include/tantivy_ohos.h", "source/Cargo.lock",
                "source/examples/basic.c", "source/examples/basic.cpp"}
    if not required <= entries.keys():
        raise ValueError(f"Missing SDK files: {required - entries.keys()}")
    checksums = {}
    for line in entries["SHA256SUMS"].decode("utf-8").splitlines():
        digest, name = line.split("  ", 1)
        if name in checksums:
            raise ValueError("Duplicate checksum entry")
        checksums[name] = digest
    if set(checksums) != entries.keys() - {"SHA256SUMS"}:
        raise ValueError("Checksum inventory differs from ZIP contents")
    for name, digest in checksums.items():
        if hashlib.sha256(entries[name]).hexdigest() != digest:
            raise ValueError(f"Checksum mismatch: {name}")
    if entries["include/tantivy_ohos.h"] != entries["source/include/tantivy_ohos.h"]:
        raise ValueError("Public header differs from packaged source header")
    manifest = json.loads(entries["BUILD-MANIFEST.json"])
    lib = entries["lib/arm64-v8a/libtantivy_ohos.so"]
    if lib[:6] != b"\x7fELF\x02\x01" or len(lib) < 64 or struct.unpack_from("<HH", lib, 16) != (3, 183):
        raise ValueError("Expected little-endian AArch64 ELF shared library")
    if manifest["target"] != "aarch64-unknown-linux-ohos" or manifest["abi_version"] != 4:
        raise ValueError("Unexpected package target or ABI")
    if manifest["library_sha256"] != hashlib.sha256(lib).hexdigest():
        raise ValueError("Library differs from build manifest")
    if not any(name.startswith("licenses/") for name in entries):
        raise ValueError("Missing dependency license files")
    return {"files": len(entries), "source_commit": manifest["source_commit"],
            "library_sha256": manifest["library_sha256"]}


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("zip")
    args = parser.parse_args()
    print(json.dumps(validate(args.zip), indent=2))
