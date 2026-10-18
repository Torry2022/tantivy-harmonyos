"""Host functional checks and external C/C++ SDK consumers.

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
import time

ROOT = Path(__file__).resolve().parents[1]
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
    report = {"platform": sys.platform, "commands": [], "passed": False}
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
        env["CARGO_TARGET_DIR"] = str(target / (label + "-" + host))
        command(["cargo", "+" + toolchain, "build", "--locked", "--release", "--lib",
                 "--target", host, "-j", args.jobs], cwd=source)
        name = "tantivy_ohos.dll" if windows else "libtantivy_ohos.so"
        destination = run / (label + "-library")
        destination.mkdir(parents=True)
        shutil.copy2(Path(env["CARGO_TARGET_DIR"]) / host / "release" / name, destination / name)
        return destination / name

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
        command([sys.executable, "-m", "unittest", "discover", "-s", "tools", "-p", "test_*.py"])
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
        writer_example = examples / ("writer_options" + (".exe" if windows else ""))
        writer_index = run / "writer-options-index"
        for phase in ["create", "reopen", "readonly"]:
            before = snapshot(writer_index) if phase == "readonly" else None
            command([writer_example, writer_index, phase], runtime=candidate.parent)
            if before is not None and snapshot(writer_index) != before:
                raise RuntimeError("Readonly writer-options check changed index files")
        acceptance = examples / ("acceptance" + (".exe" if windows else ""))
        acceptance_index = run / "acceptance-index"
        for phase in ["create", "reopen", "readonly"]:
            before = snapshot(acceptance_index) if phase == "readonly" else None
            command([acceptance, acceptance_index, phase], runtime=candidate.parent)
            if before is not None and snapshot(acceptance_index) != before:
                raise RuntimeError("Readonly acceptance changed index files")
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
