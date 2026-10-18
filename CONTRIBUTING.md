# Contributing

Use an issue to describe a bug or a proposed public API change. Include the SDK release, host and target, Rust and Native SDK versions, a minimal example, and the full error. For index problems, include field/tokenizer configuration and the sequence of write/commit/reload calls. Do not upload private documents, application data or signing files.

## Run the checks

The host runner needs Python 3.12+, Rust 1.90.0, CMake 3.20+, Ninja, Git with full history, and GNU/Clang C and C++ compilers. On Linux:

```sh
rustup toolchain install 1.90.0 --profile minimal
python3 tools/check.py
```

It builds the two pinned published versions and the working tree, runs Rust tests, builds/runs the C99 and C++11 examples with CMake, and runs unchanged historical consumers with the candidate library. It also checks exported functions, frozen declarations/layouts, and read-only access to indexes created by the historical libraries. Dependencies use their committed lock files. Historical sources come from local Git objects, not downloaded binaries. A shallow clone must fetch the referenced history first.

Windows uses the Rust `1.90.0-x86_64-pc-windows-gnu` toolchain, llvm-mingw, CMake and Ninja on PATH. The GNU linker may need `CARGO_TARGET_X86_64_PC_WINDOWS_GNU_LINKER` set to the matching Rust bundled GCC driver. Use `--cc`, `--cxx`, `--cmake` and `--readobj` for tool paths containing spaces. The runner selects this explicit GNU toolchain on Windows and `+1.90.0` on Linux without changing global toolchain settings. It does not cover MSVC.

Each run retains `build/check-<timestamp>-<pid>/report.json`, command logs and its own fixtures. The reusable compilation cache is `build/check-target/`. Run one instance per checkout. A failure is a failure: there are no automatic retries or skipped failed cases. Windows file replacement may encounter system error 5 when another process holds an index file; preserve the failing path/log rather than changing indexing semantics to hide it.

GitHub Actions runs the Linux host entry point and uploads its reports and logs, including failed runs. This is host coverage, not an OHOS build or device certification.

## Check an OHOS artifact

Build using `build_ohos.ps1` as described in the README. Native SDK changes, native implementation changes or ABI changes also need an independent HarmonyOS application check, using the matching library/header and an index isolated from other applications. Use `prepare_api_smoke.ps1`; keep signing material local. Do not reuse historical device records to approve a changed binary.

After packaging:

```sh
python3 tools/check_package.py path/to/tantivy-0.26.2-harmonyos-arm64-v8a.zip
```

This checks the complete checksum inventory, required files, matching public/source headers, build-manifest library hash and AArch64 ELF type. It does not prove the library was built from the packaged source, perform a license audit or execute code. Review the clean source commit, build logs, dependency notices and actual exported symbols before release; `package_release.py` still requires the explicitly reviewed library hash.

## Changing the interface

Read [compatibility and support](docs/COMPATIBILITY.md) first. Preserve frozen C layouts and legacy behavior; use additive symbols for incompatible options. New APIs need an ownership/thread/error contract and external C/C++ tests. Changes to schema, dictionaries or saved metadata need old-index checks. Keep the two READMEs aligned and add a concise user-facing changelog entry. Formatting or documentation-only changes do not require device installation or a new binary release.
