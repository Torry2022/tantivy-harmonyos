# tantivy-harmonyos

[简体中文](README.md) | [English](README_EN.md)

[Tantivy](https://github.com/quickwit-oss/tantivy) full-text search for HarmonyOS arm64 applications, exposed through a C API for Native modules. Supports Jieba Chinese tokenization, BM25 ranking, Boolean and phrase queries, and persistent indexes with inserts, updates, and deletes.

This unofficial port uses Tantivy 0.26.2 and jieba-rs 0.8.1 without patches to Tantivy itself. The adapter has fixed document ID and text fields for document retrieval; it does not expose Tantivy's entire API.

## Download

Download `tantivy-0.26.2-harmonyos-arm64-v8a.zip` from [Releases](https://github.com/Torry2022/tantivy-harmonyos/releases). It includes the shared library, C header, source, test project, and dependency notices. The Chinese dictionary is embedded in the library.

Check the download against the accompanying `SHA256SUMS`:

```powershell
Get-FileHash .\tantivy-0.26.2-harmonyos-arm64-v8a.zip -Algorithm SHA256
```

In release numbers, `0.26.2` is the Tantivy version and `ohos.1` is this project's port revision.

## Usage

Place `libtantivy_ohos.so` in the module's `entry/src/main/libs/arm64-v8a/` directory and add `tantivy_ohos.h` to the header search path. Link the library in CMake (assuming the calling target is named `entry`):

```cmake
add_library(tantivy_ohos SHARED IMPORTED)
set_target_properties(tantivy_ohos PROPERTIES
    IMPORTED_LOCATION "${CMAKE_CURRENT_LIST_DIR}/../libs/arm64-v8a/libtantivy_ohos.so")
target_link_libraries(entry PRIVATE tantivy_ohos)
```

Keep indexes in the application's writable sandbox. Obtain `filesDir` in ArkTS and pass it to the Native module. The basic call sequence is:

```text
open(create=true) → upsert → commit → search → close
open(create=false) → search → delete / upsert → commit → close
```

See [tantivy_ohos.h](include/tantivy_ohos.h) for the interface and [tantivy_smoke.cpp](smoke/tantivy_smoke.cpp) for a complete example. When using the API:

- Creating an index requires a path that does not yet exist, with an existing parent directory. Existing indexes must match the adapter's schema and tokenizer configuration.
- Inserts and deletes become searchable after `commit`, which also reloads the handle's reader. `close` does not commit pending changes.
- Only one writer may open an index. Serialize all operations on a handle, including close.
- Search returns a JSON array of `id`, `text`, and `score` objects. Free both results and error strings with `tantivy_ohos_free_string`.

Chinese tokenization uses Jieba's default dictionary and mode with HMM, followed by lowercase normalization. Queries use Tantivy QueryParser syntax: space-separated terms default to AND, with explicit Boolean operators and double-quoted phrases also supported. Custom fields and domain dictionaries are not currently exposed by the API.

## Build from source

Requires Windows, Rust 1.90.0 with the GNU toolchain, 64-bit MinGW tools, and the DevEco OpenHarmony Native SDK. The tested MinGW tools are [llvm-mingw 20261006 UCRT x86_64](https://github.com/mstorsjo/llvm-mingw/releases/tag/20261006).

In PowerShell, from the repository directory, prepare Rust:

```powershell
rustup toolchain install 1.90.0-x86_64-pc-windows-gnu --profile minimal
rustup target add aarch64-unknown-linux-ohos --toolchain 1.90.0-x86_64-pc-windows-gnu
$env:RUSTUP_TOOLCHAIN = '1.90.0-x86_64-pc-windows-gnu'
```

Build with the paths to your SDK and MinGW installation:

```powershell
.\build_ohos.ps1 `
  -OpenHarmonySdk 'D:\path\to\DevEco Studio\sdk\default\openharmony' `
  -HostLlvm 'D:\path\to\llvm-mingw-20261006-ucrt-x86_64'
```

The script builds with `Cargo.lock` and two parallel jobs by default. The target library is linked with OHOS Clang and its sysroot. Output in `out/aarch64-unknown-linux-ohos/`:

```text
libtantivy_ohos.so
tantivy_ohos.h
```

Run host tests and prepare the device test project:

```powershell
cargo test --locked --release -j 2
.\prepare_smoke.ps1
```

Then open `smoke/harmony` in DevEco, configure debug signing, and install it. See [VALIDATION.md](VALIDATION.md) for test instructions, device results, and untested scenarios. HarmonyOS arm64 has been validated; other architectures and system versions may need further work.

## License

The adapter and scripts use [MIT](LICENSE). Tantivy, jieba-rs, and other dependencies retain their own licenses; see [NOTICE](NOTICE) and [third_party/](third_party/). The prebuilt archive includes dependency, Rust standard-library, and dictionary notices in `licenses/`, with build provenance in `BUILD-MANIFEST.json`. Include these materials when redistributing.

Maintainers can run `python package_release.py` in the build environment to generate a release archive under `build/release/`.
