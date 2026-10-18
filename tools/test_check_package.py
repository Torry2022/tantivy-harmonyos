import hashlib
import json
from pathlib import Path
import struct
import tempfile
import unittest
import zipfile
from check_package import validate


class PackageChecks(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory()
        self.addCleanup(self.temp.cleanup)
        self.path = Path(self.temp.name) / "sdk.zip"
        lib = bytearray(64)
        lib[:6] = b"\x7fELF\x02\x01"
        struct.pack_into("<HH", lib, 16, 3, 183)
        self.files = {name: b"fixture" for name in ["LICENSE", "NOTICE",
            "include/tantivy_ohos.h", "source/include/tantivy_ohos.h",
            "source/Cargo.lock", "source/examples/basic.c", "source/examples/basic.cpp",
            "licenses/dependency/LICENSE"]}
        self.files["lib/arm64-v8a/libtantivy_ohos.so"] = lib
        self.files["BUILD-MANIFEST.json"] = json.dumps({"abi_version": 4,
            "target": "aarch64-unknown-linux-ohos", "source_commit": "fixture",
            "library_sha256": hashlib.sha256(lib).hexdigest()}).encode()

    def write(self, corrupt=False):
        files = self.files.copy()
        files["SHA256SUMS"] = "".join(hashlib.sha256(value).hexdigest() + "  " + name + "\n"
                                     for name, value in files.items()).encode()
        if corrupt:
            files["NOTICE"] = b"changed after checksums"
        with zipfile.ZipFile(self.path, "w") as archive:
            for name, value in files.items():
                archive.writestr("sdk/" + name, value)

    def test_consistent_package(self):
        self.write()
        self.assertEqual(validate(self.path)["source_commit"], "fixture")

    def test_corruption_fails(self):
        self.write(corrupt=True)
        with self.assertRaisesRegex(ValueError, "Checksum mismatch"):
            validate(self.path)

    def test_missing_header_fails_even_with_valid_checksums(self):
        del self.files["include/tantivy_ohos.h"]
        self.write()
        with self.assertRaisesRegex(ValueError, "Missing SDK files"):
            validate(self.path)

    def test_different_source_header_fails(self):
        self.files["source/include/tantivy_ohos.h"] = b"wrong header"
        self.write()
        with self.assertRaisesRegex(ValueError, "header differs"):
            validate(self.path)


if __name__ == "__main__":
    unittest.main()
