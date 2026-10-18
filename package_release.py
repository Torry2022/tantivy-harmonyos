"""Package the built OHOS library, tracked sources and pinned dependency notices."""
import argparse
import hashlib
import json
import pathlib
import subprocess
import zipfile

ROOT = pathlib.Path(__file__).resolve().parent
TARGET = "aarch64-unknown-linux-ohos"


def command(*args):
    return subprocess.check_output(args, cwd=ROOT, text=True, encoding="utf-8").strip()


def digest(data):
    return hashlib.sha256(data).hexdigest()


def is_notice(path):
    name = path.name.lower()
    return any(word in name for word in
               ("license", "licence", "copying", "copyright", "notice", "unlicense"))


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--expected-library-sha256", required=True,
                        help="SHA-256 of the reviewed library to package; not a device-validation assertion")
    args = parser.parse_args()
    expected = args.expected_library_sha256.lower()
    if len(expected) != 64 or any(c not in "0123456789abcdef" for c in expected):
        raise SystemExit("Expected library hash must be 64 hexadecimal characters")
    if command("git", "status", "--porcelain"):
        raise SystemExit("Review and commit all source changes before packaging")
    metadata = json.loads(command("cargo", "metadata", "--locked", "--format-version", "1",
                                  "--filter-platform", TARGET))
    commit = command("git", "rev-parse", "HEAD")
    files = {}
    dependencies = []
    supplemental = ROOT / "third_party" / "supplemental"
    for package in metadata["packages"]:
        if not package["source"]:
            continue
        name = f"{package['name']}-{package['version']}"
        source = pathlib.Path(package["manifest_path"]).parent
        notices = [p for p in source.rglob("*") if p.is_file() and is_notice(p)]
        extra = supplemental / name
        if extra.exists():
            for path in extra.iterdir():
                files[f"licenses/crates/{name}/{path.name}"] = path.read_bytes()
        if not notices and not extra.exists():
            raise SystemExit(f"Missing license material: {name}")
        for path in notices:
            files[f"licenses/crates/{name}/{path.relative_to(source).as_posix()}"] = path.read_bytes()
        dependencies.append({"name": package["name"], "version": package["version"],
                             "license": package["license"], "repository": package["repository"],
                             "authors": package["authors"]})
    # Compiler/runtime code is statically linked; host LLVM/MinGW is not shipped.
    rust_docs = pathlib.Path(command("rustc", "--print", "sysroot")) / "share/doc/rust"
    runtime_notice = rust_docs / "COPYRIGHT-library.html"
    files["licenses/rust/COPYRIGHT-library.html"] = runtime_notice.read_bytes()
    for path in (rust_docs / "licenses").rglob("*"):
        if path.is_file():
            files[f"licenses/rust/licenses/{path.relative_to(rust_docs / 'licenses').as_posix()}"] = path.read_bytes()
    for path in (ROOT / "third_party").rglob("*"):
        if path.is_file():
            files[f"licenses/{path.relative_to(ROOT / 'third_party').as_posix()}"] = path.read_bytes()
    dictionary = {}
    for package, relative in [("jieba-rs", "src/data/dict.txt"),
                              ("jieba-macros", "src/hmm.model")]:
        crate = next(p for p in metadata["packages"] if p["name"] == package)
        data = (pathlib.Path(crate["manifest_path"]).parent / relative).read_bytes()
        dictionary[f"{package}-{crate['version']}/{relative}"] = digest(data)
    for relative in command("git", "ls-files").splitlines():
        files[f"source/{relative}"] = (ROOT / relative).read_bytes()
    for relative in ["docs/VALIDATION.md", "LICENSE", "NOTICE", "Cargo.lock"]:
        files[relative] = (ROOT / relative).read_bytes()
    files["README.md"] = ("# tantivy-harmonyos\n\n"
        "[简体中文](README.md) | [English](README_EN.md)\n\n"
        "HarmonyOS arm64 全文检索 C SDK。\n\n"
        "- 动态库：`lib/arm64-v8a/libtantivy_ohos.so`\n"
        "- 配套头文件：`include/tantivy_ohos.h`\n"
        "- [接入与构建说明](source/README.md)、[调用示例](source/examples/)\n"
        "- [验证范围](docs/VALIDATION.md)、[许可](LICENSE)和[依赖声明](NOTICE)\n\n"
        "请将库和头文件配套使用。源码构建命令在 `source/` 目录执行。"
        "包内示例未签名，运行 HarmonyOS 测试应用需配置自己的调试 Profile。\n").encode()
    files["README_EN.md"] = ("# tantivy-harmonyos\n\n"
        "[简体中文](README.md) | [English](README_EN.md)\n\n"
        "Full-text search C SDK for HarmonyOS arm64.\n\n"
        "- Shared library: `lib/arm64-v8a/libtantivy_ohos.so`\n"
        "- Matching header: `include/tantivy_ohos.h`\n"
        "- [Integration and build instructions](source/README_EN.md), [examples](source/examples/)\n"
        "- [Validation scope](docs/VALIDATION.md), [license](LICENSE) and [dependency notices](NOTICE)\n\n"
        "Use the library with its matching header. Run source build commands from `source/`. "
        "The sample is unsigned; running a HarmonyOS test app requires your own debug Profile.\n").encode()
    files["include/tantivy_ohos.h"] = (ROOT / "include/tantivy_ohos.h").read_bytes()
    library = (ROOT / "out" / TARGET / "libtantivy_ohos.so").read_bytes()
    files["lib/arm64-v8a/libtantivy_ohos.so"] = library
    # Match the explicitly reviewed library build.
    if digest(library) != expected:
        raise SystemExit("Library differs from the explicitly reviewed artifact")
    adapter = next(p for p in metadata["packages"] if p["name"] == "tantivy-ohos")
    manifest = {"source_commit": commit, "target": TARGET, "tantivy": "0.26.2",
                "adapter": adapter["version"], "interface": "unified",
                "device_validation": "Not asserted by packaging; see validation records",
                "rust": command("rustc", "--version"),
                "library_sha256": digest(library), "dictionary_inputs": dictionary,
                "dependencies": dependencies,
                "license_scope": "Resolved OHOS graph, including build-time dependencies; "
                                 "not every listed crate is linked into the final library."}
    files["BUILD-MANIFEST.json"] = (json.dumps(manifest, ensure_ascii=False, indent=2) + "\n").encode()
    files["SHA256SUMS"] = "".join(f"{digest(data)}  {name}\n"
                                  for name, data in sorted(files.items())).encode()
    output = ROOT / "build/release"
    output.mkdir(parents=True, exist_ok=True)
    archive = output / "tantivy-0.26.2-harmonyos-arm64-v8a.zip"
    prefix = archive.stem + "/"
    with zipfile.ZipFile(archive, "w", zipfile.ZIP_DEFLATED, compresslevel=9) as zipped:
        for name, data in sorted(files.items()):
            info = zipfile.ZipInfo(prefix + name, (2026, 10, 17, 0, 0, 0))
            info.compress_type = zipfile.ZIP_DEFLATED
            info.external_attr = 0o100644 << 16
            zipped.writestr(info, data)
    checksum = f"{digest(archive.read_bytes())}  {archive.name}\n"
    (output / "SHA256SUMS").write_text(checksum, encoding="ascii")
    print(f"Packaged {len(dependencies)} dependency notices; {len(files)} files")
    print(checksum, end="")


if __name__ == "__main__":
    main()
