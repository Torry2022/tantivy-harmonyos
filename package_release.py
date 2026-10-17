"""Package the built OHOS library, tracked sources and pinned dependency notices."""
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
    if command("git", "status", "--porcelain", "--untracked-files=no"):
        raise SystemExit("Commit tracked changes before packaging")
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
    for relative in ["README.md", "VALIDATION.md", "LICENSE", "NOTICE", "Cargo.lock"]:
        files[relative] = (ROOT / relative).read_bytes()
    files["include/tantivy_ohos.h"] = (ROOT / "include/tantivy_ohos.h").read_bytes()
    library = (ROOT / "out" / TARGET / "libtantivy_ohos.so").read_bytes()
    files["lib/arm64-v8a/libtantivy_ohos.so"] = library
    # This release packages the exact library already validated on the device.
    expected = "465e2765fbf721395ce0e32667764476fc90a61db67c9b306fb57a9390c97938"
    if digest(library) != expected:
        raise SystemExit("Library differs from the validated first-release artifact")
    manifest = {"source_commit": commit, "target": TARGET, "tantivy": "0.26.2",
                "adapter": "0.1.0", "rust": command("rustc", "--version"),
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
