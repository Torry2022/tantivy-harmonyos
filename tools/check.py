"""Host regression, frozen C ABI, and published-index compatibility checks.

Requires Python 3.12+, Rust 1.90.0, CMake, Ninja and a GNU/Clang C/C++ toolchain.
No retries. Each invocation retains logs, fixtures and a report under build/.
"""
import argparse
import hashlib
import json
import os
from pathlib import Path
import re
import shutil
import subprocess
import sys
import tarfile
import time

ROOT = Path(__file__).resolve().parents[1]
BASELINES = {
    "abi1": "29c0379fcf1f6df0db38a84979487d0fe3b03ba1",
    "abi4": "20851205341fb12cb44db89c0656fe9800042a6f",
}


def header_contract(text):
    """Freeze declarations, not comments or source formatting."""
    text = re.sub(r"/\*.*?\*/|//[^\n]*", "", text, flags=re.S)
    declarations = re.findall(
        r"typedef\s+struct\s*\{[^}]*\}\s*\w+\s*;|"
        r"typedef\s+struct\s+\w+\s+\w+\s*;|"
        r"(?:int|void|uint32_t)\s+tantivy_ohos_\w+\s*\([^;]*?\)\s*;|"
        r"#define[ \t]+TANTIVY_OHOS_\w+[ \t]+[^\n]+", text)
    def normalize(value):
        return re.sub(r"\s*([*{},();])\s*", r"\1", " ".join(value.split()))
    return sorted(normalize(value) for value in declarations)


def check_contract(text, frozen):
    missing = set(frozen) - set(header_contract(text))
    if missing:
        raise ValueError("Changed/removed frozen declarations:\n" + "\n".join(sorted(missing)))


def snapshot(directory):
    return {p.relative_to(directory).as_posix(): {
        "size": p.stat().st_size, "mtime_ns": p.stat().st_mtime_ns,
        "sha256": hashlib.sha256(p.read_bytes()).hexdigest()
    } for p in sorted(directory.rglob("*")) if p.is_file()}


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--cc", default=os.environ.get("CC", "cc"))
    parser.add_argument("--cxx", default=os.environ.get("CXX", "c++"))
    parser.add_argument("--cmake", default="cmake")
    parser.add_argument("--readobj", default="llvm-readobj",
                        help="Windows PE export inspection tool")
    parser.add_argument("--jobs", type=int, default=2)
    args = parser.parse_args()
    if sys.version_info < (3, 12) or args.jobs < 1:
        parser.error("Python 3.12+ and positive --jobs required")
    windows = sys.platform == "win32"
    toolchain = "1.90.0-x86_64-pc-windows-gnu" if windows else "1.90.0"
    if not windows and not sys.platform.startswith("linux"):
        parser.error("This runner currently covers Linux and Windows GNU hosts")
    run = ROOT / "build" / ("check-" + time.strftime("%Y%m%d-%H%M%S") + f"-{os.getpid()}")
    run.mkdir(parents=True, exist_ok=False)
    report = {"platform": sys.platform, "baselines": BASELINES, "commands": [], "passed": False}
    env = os.environ.copy()
    target = ROOT / "build" / "check-target"
    env["CARGO_TARGET_DIR"] = str(target)

    def command(argv, cwd=ROOT, runtime=None):
        log = run / f"{len(report['commands']):02d}.log"
        record = {"argv": [str(a) for a in argv], "cwd": str(cwd), "log": log.name}
        report["commands"].append(record)
        print(subprocess.list2cmdline(record["argv"]), flush=True)
        started = time.monotonic()
        call_env = env.copy()
        if runtime:
            key = "PATH" if windows else "LD_LIBRARY_PATH"
            call_env[key] = str(runtime) + os.pathsep + call_env.get(key, "")
        try:
            with log.open("w", encoding="utf-8") as output:
                result = subprocess.run(record["argv"], cwd=cwd, env=call_env,
                                        stdout=output, stderr=subprocess.STDOUT, timeout=1200)
            record["returncode"] = result.returncode
            if result.returncode:
                raise RuntimeError(f"exit {result.returncode}: {log}")
            return log.read_text(encoding="utf-8", errors="replace")
        finally:
            record["seconds"] = round(time.monotonic() - started, 3)

    def library(source, label):
        # Cargo root-package fingerprints can collide between archived workspaces
        # with identical package names/versions. Keep each baseline independent.
        env["CARGO_TARGET_DIR"] = str(target / (label + "-" + host))
        command(["cargo", "+" + toolchain, "build", "--locked", "--release", "--lib",
                 "--target", host, "-j", args.jobs], cwd=source)
        name = "tantivy_ohos.dll" if windows else "libtantivy_ohos.so"
        destination = run / (label + "-library")
        destination.mkdir(parents=True)
        shutil.copy2(Path(env["CARGO_TARGET_DIR"]) / host / "release" / name, destination / name)
        return destination / name

    def compile_consumer(source, header, lib, name):
        executable = run / (name + (".exe" if windows else ""))
        command([args.cc, "-std=c99", "-Wall", "-Wextra", "-Werror",
                 "-I", header, source, "-L", lib.parent, "-l:" + lib.name, "-o", executable])
        return executable

    try:
        required = [args.cc, args.cxx, args.cmake, "ninja", "git", "cargo", "rustc",
                    args.readobj if windows else "nm"]
        for program in required:
            if not shutil.which(program):
                raise RuntimeError(f"Required tool not found: {program}")
        report["source_commit"] = command(["git", "rev-parse", "HEAD"]).strip()
        report["working_changes"] = command(["git", "status", "--porcelain"])
        version = command(["rustc", "+" + toolchain, "-vV"])
        host = re.search(r"^host: (.+)$", version, re.M).group(1)
        if windows and not host.endswith("windows-gnu"):
            raise RuntimeError("Windows checks require the GNU Rust toolchain")
        report["host"] = host
        frozen = json.loads((ROOT / "tests/abi4-contract.json").read_text(encoding="utf-8"))
        check_contract((ROOT / "include/tantivy_ohos.h").read_text(encoding="utf-8"),
                       frozen["declarations"])
        command([sys.executable, "-m", "unittest", "discover", "-s", "tools", "-p", "test_*.py"])
        historical = {}
        for name, revision in BASELINES.items():
            source = run / name
            source.mkdir()
            archive = run / f"{name}.tar"
            # A full Git checkout supplies pinned sources; there is no remote fetch.
            command(["git", "-c", "core.autocrlf=false", "archive", "--format=tar",
                     f"--output={archive}", revision])
            with tarfile.open(archive) as tar:
                tar.extractall(source, filter="data")
            if name == "abi4":
                original = (source / "include/tantivy_ohos.h").read_bytes()
                if hashlib.sha256(original).hexdigest() != frozen["header_sha256"]:
                    raise RuntimeError("Frozen header provenance mismatch")
            lib = library(source, name)
            report[name + "_library_sha256"] = hashlib.sha256(lib.read_bytes()).hexdigest()
            consumer_source = ROOT / "tests" / ("legacy_abi.c" if name == "abi1" else "compat_v4.c")
            exe = compile_consumer(consumer_source, source / "include", lib, name + "-consumer")
            index = run / (name + "-index")
            command([exe, index, "create"], runtime=lib.parent)
            command([exe, index, "read"], runtime=lib.parent)
            historical[name] = (exe, index)

        # Build and test the candidate only after historical artifacts have been copied.
        env["CARGO_TARGET_DIR"] = str(target / ("candidate-" + host))
        command(["cargo", "+" + toolchain, "test", "--locked", "--release", "--target", host,
                 "-j", args.jobs])
        candidate = library(ROOT, "candidate")
        report["library_sha256"] = hashlib.sha256(candidate.read_bytes()).hexdigest()
        names = set(re.findall(r"\b(tantivy_ohos_\w+)\s*\(",
                               (ROOT / "include/tantivy_ohos.h").read_text(encoding="utf-8")))
        if windows:
            exports = command([args.readobj, "--coff-exports", candidate])
            exported = set(re.findall(r"Name: (tantivy_ohos_\w+)", exports))
        else:
            exports = command(["nm", "-D", "--defined-only", candidate])
            exported = set(re.findall(r"\b(tantivy_ohos_\w+)\b", exports))
        if names != exported:
            raise RuntimeError(f"Header/exports differ: missing={names-exported}, extra={exported-names}")
        report["exports"] = sorted(exported)
        examples = run / "examples"
        command([args.cmake, "-S", ROOT / "examples", "-B", examples, "-G", "Ninja",
                 f"-DTANTIVY_INCLUDE_DIR={(ROOT / 'include').as_posix()}",
                 f"-DTANTIVY_LIBRARY={candidate.as_posix()}",
                 f"-DCMAKE_C_COMPILER={Path(shutil.which(args.cc) or args.cc).resolve().as_posix()}",
                 f"-DCMAKE_CXX_COMPILER={Path(shutil.which(args.cxx) or args.cxx).resolve().as_posix()}",
                 "-DCMAKE_BUILD_TYPE=Release", "-DTANTIVY_WARNINGS_AS_ERRORS=ON"])
        command([args.cmake, "--build", examples, "--parallel", args.jobs])
        for example in ["basic_c", "basic_cpp", "advanced"]:
            exe = examples / (example + (".exe" if windows else ""))
            command([exe, run / (example + "-index")], runtime=candidate.parent)
        current_reader = compile_consumer(ROOT / "tests/compat_v4.c", ROOT / "include",
                                          candidate, "current-reader")
        for name, (_, index) in historical.items():
            before = snapshot(index)
            command([current_reader, index, "read-legacy" if name == "abi1" else "read"],
                    runtime=candidate.parent)
            after = snapshot(index)
            if after != before:
                raise RuntimeError(f"Readonly access changed {name} index files")
            (run / (name + "-readonly.json")).write_text(json.dumps(after, indent=2) + "\n")
        for name, (exe, index) in historical.items():
            # First prove readonly access to untouched old indexes above. ABI 1's
            # original reader below uses read/write open; it must not mask upgrades.
            # Do not relink/recompile: change only the loaded shared library.
            before = snapshot(index)
            binary_hash = hashlib.sha256(exe.read_bytes()).hexdigest()
            command([exe, index, "read"], runtime=candidate.parent)
            if name == "abi4" and snapshot(index) != before:
                raise RuntimeError("ABI 4 readonly compatibility check modified index files")
            report[name + "_consumer_sha256"] = hashlib.sha256(exe.read_bytes()).hexdigest()
            if report[name + "_consumer_sha256"] != binary_hash:
                raise RuntimeError("Frozen consumer executable changed")
        report["passed"] = True
    except Exception as error:
        report["error"] = str(error)
        print(f"FAILED: {error}", file=sys.stderr)
        if report["commands"]:
            log = run / report["commands"][-1]["log"]
            if log.exists():
                print(log.read_text(encoding="utf-8", errors="replace")[-6000:], file=sys.stderr)
    finally:
        (run / "report.json").write_text(json.dumps(report, ensure_ascii=False, indent=2) + "\n",
                                         encoding="utf-8")
        print(f"{'PASS' if report['passed'] else 'FAIL'}: {run / 'report.json'}", flush=True)
    return 0 if report["passed"] else 1


if __name__ == "__main__":
    sys.exit(main())
