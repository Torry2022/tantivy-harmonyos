# tantivy-harmonyos

[简体中文](README.md) | [English](README_EN.md)

An unofficial HarmonyOS arm64 port of Tantivy 0.26.2, with reproducible Windows PowerShell builds, a small C API, Chinese tokenization, and a standalone device test project.

This project helps validate local full-text search in HarmonyOS Native modules. It is not an official Tantivy or Huawei project. The first adapter uses fixed document ID and text fields. It is independent of Turbo AI Chat and does not require a vector database.

## Features

- Jieba Chinese tokenization with UTF-8 byte offsets and lowercase normalization; the dictionary is embedded in the library.
- BM25 ranking, Chinese and mixed-language queries, Boolean queries, and phrase queries.
- Persistent indexes: create/open, upsert by ID, delete, and commit.
- C ABI results containing document IDs, text, and scores as JSON, with matching functions to free results and error strings.
- A standalone test HAP covering ArkTS → Node-API / C++ → Rust → Tantivy.

This is not a binding for Tantivy's entire API. See the [C header](include/tantivy_ohos.h) for query syntax, tokenization, and field definitions. Existing indexes must match this adapter's schema.

## Prebuilt library

Download `tantivy-0.26.2-harmonyos-arm64-v8a.zip` from [Release v0.26.2-ohos.1](https://github.com/Torry2022/tantivy-harmonyos/releases/tag/v0.26.2-ohos.1) and check it against the accompanying `SHA256SUMS`:

```powershell
Get-FileHash .\tantivy-0.26.2-harmonyos-arm64-v8a.zip -Algorithm SHA256
```

The archive contains the arm64 library, C header, source and unsigned test project, build manifest, and third-party notices. No separate dictionary download is needed. Follow the Native integration instructions below. The release number follows upstream Tantivy; `ohos.1` identifies the first port release. The C API adapter version is 0.1.0.

## Build

The validated setup uses Windows, Rust 1.90.0 with a GNU host, the DevEco OpenHarmony Native SDK, and the arm64 OHOS target. Tantivy and jieba-rs are pinned to 0.26.2 and 0.8.1; Cargo.lock pins the full dependency graph.

Install Rust 1.90.0 and the target standard library:

```powershell
rustup toolchain install 1.90.0 --profile minimal
rustup target add aarch64-unknown-linux-ohos --toolchain 1.90.0
```

The Windows GNU host needs working 64-bit MinGW linker tools. Testing used [llvm-mingw 20261006 UCRT x86_64](https://github.com/mstorsjo/llvm-mingw/releases/tag/20261006); a 32-bit-only MinGW installation is insufficient. Host linker tools and the host Rust standard library are not OHOS target libraries. The target binary is linked with OHOS Clang and its sysroot.

```powershell
.\build_ohos.ps1 `
  -OpenHarmonySdk 'D:\path\to\DevEco Studio\sdk\default\openharmony' `
  -HostLlvm 'D:\path\to\llvm-mingw-20261006-ucrt-x86_64'
```

The script uses `cargo rustc --locked`, with at most two parallel jobs by default. It configures tools only for the current process and restores environment variables on exit. It does not change the system PATH, download upstream sources, or modify them. Output:

```text
out/aarch64-unknown-linux-ohos/
  libtantivy_ohos.so
  tantivy_ohos.h
```

The linker explicitly sets the `libtantivy_ohos.so` SONAME so CMake does not encode a development-machine path in consumers. No upstream Tantivy source patches are required. Transitive compression dependencies still exist; the script configures OHOS C/C++ compilers and the archiver for them.

## Native integration

Add the header directory to your include paths and place the library in `entry/src/main/libs/arm64-v8a/`. Import and link it in CMake:

```cmake
add_library(tantivy_ohos SHARED IMPORTED)
set_target_properties(tantivy_ohos PROPERTIES
    IMPORTED_LOCATION "${CMAKE_CURRENT_LIST_DIR}/../libs/arm64-v8a/libtantivy_ohos.so")
target_link_libraries(entry PRIVATE tantivy_ohos)
```

The final HAP must include the library. Keep indexes in the application's writable sandbox: obtain filesDir in ArkTS and pass it to Native code. Do not hardcode another application's sandbox path.

Call sequence:

```text
open(create=true) → upsert → commit → search → close
open(create=false) → search → delete / upsert → commit → search → close
```

- `create=true` requires a new directory and does not overwrite an existing one. Its parent directory must already exist.
- `commit` also reloads this handle's reader. Uncommitted changes are not searchable, and close does not implicitly commit.
- Only one writer may open an index. Callers must serialize all operations on a handle, including close.
- Queries use Tantivy QueryParser with AND as the default operator. Syntax errors are returned as errors.
- Free returned strings with `tantivy_ohos_free_string`, never with `free`, `delete`, or an allocator outside Rust.
- Scores are meaningful within their query and configuration; do not add them directly to vector similarity scores.

## Tests

Host functional tests:

```powershell
cargo test --locked --release -j 2
```

For the GNU host, configure the 64-bit tools above in the current terminal. Prepare the device test project:

```powershell
.\prepare_smoke.ps1
```

Open `smoke/harmony` in DevEco, configure the test package's own debug Profile, and install it. An existing debug certificate and key can be reused; a Profile for a new bundle ID does not require a new debug certificate. Do not borrow a business application's bundle ID or overwrite an app being used for experiments.

The default test bundle ID is `com.torry2022.retrievalprobe`. The three phases share an index and must run in separate processes:

```sh
hdc -t <device> shell aa start -a EntryAbility -b com.torry2022.retrievalprobe --ps phase create
hdc -t <device> shell aa force-stop com.torry2022.retrievalprobe
hdc -t <device> shell aa start -a EntryAbility -b com.torry2022.retrievalprobe --ps phase reopen
hdc -t <device> shell aa force-stop com.torry2022.retrievalprobe
hdc -t <device> shell aa start -a EntryAbility -b com.torry2022.retrievalprobe --ps phase verify
```

Tests use filesDir/trial-01 by default. To repeat them without deleting old data, use the same fresh `--ps runId trial-02` in all three phase commands. The sample does not delete existing indexes. Reports are saved in that trial directory as `report-tantivy-*.txt`, `details-tantivy-*.log`, and `errors-tantivy-*.log`.

See [smoke/tantivy_smoke.cpp](smoke/tantivy_smoke.cpp) for test code and [VALIDATION.md](VALIDATION.md) for device results and coverage boundaries.

## Limitations

- Only arm64 has been validated; this does not establish x86_64 emulator support or compatibility with every HarmonyOS version.
- No vector retrieval, hybrid ranking fusion, Embedding model, or product UI is included.
- Tokenization uses Jieba default mode with HMM, without a domain dictionary, search-mode synonyms, or relevance tuning.
- Index compatibility across Tantivy versions, schemas, or tokenizer configurations is not guaranteed.
- Small correctness tests do not establish performance, retrieval quality, power-loss recovery, or concurrency robustness.
- The tested library dynamically depends only on system libc. Rust and other dependencies are statically included, so their notices must be retained when distributing it. The prebuilt archive includes dependency licenses and provenance.

## License

The adapter and scripts use MIT. Tantivy, jieba-rs, and other dependencies retain their own licenses; see LICENSE, NOTICE, third_party/, and Cargo.lock. The prebuilt archive includes dependency, Rust standard-library, and dictionary notices under `licenses/`. `BUILD-MANIFEST.json` records the source commit, dependency versions, and dictionary hashes. Retain these materials when redistributing.

Maintainers can run `python package_release.py` in the same Rust environment used for the build (Python 3 required). The script collects notices from the locked OHOS dependency graph, adds the validated library, Git-tracked source and file checksums, and writes to `build/release/`. Local signing materials and build directories are excluded.
