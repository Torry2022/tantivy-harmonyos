# Compatibility and support

## Version numbers

| Identifier | Meaning |
| --- | --- |
| `v0.26.2-ohos.2` | Tantivy 0.26.2 plus this project's second adaptation release; not an upstream Tantivy release |
| C ABI `4` | Cumulative supported C interface, including ABI 1/2/3 symbols; consumers require at least the ABI they use |
| v2 options | Frozen layouts; `version=2`, with `struct_size=sizeof(the exact type)` even on ABI 4 |
| v3 creation/search JSON | Request and saved-configuration format, independent of the C ABI number |
| Cargo package `0.1.0` | Adapter crate metadata; not the binary release or C ABI version |
| Tantivy index format | Upstream on-disk representation; distinct from adapter metadata and C ABI |

An ABI-compatible library is not automatically compatible with every upstream index format. Existing supported indexes open without automatic schema or dictionary migration. Missing `fast_id` metadata keeps the stored-ID fallback. Changing fields or analyzers requires a new index and reindexing. Before upgrading Tantivy, verify old index fixtures against the candidate library; document incompatibility and a supported rebuild route if necessary. Never silently rewrite an index on read-only open.

## Current build and validation scope

- Distribution: AArch64 `aarch64-unknown-linux-ohos`, shared library with a C interface. No distributed x86, Windows or macOS SDK binaries.
- Locked dependencies: Tantivy 0.26.2 and jieba-rs 0.8.1; Rust 1.90.0. The reference Windows OHOS build uses llvm-mingw 20261006 UCRT and OpenHarmony Native SDK API 26 (26.0.0.105) bundled with DevEco. This is a build configuration, not a minimum supported device API.
- ABI 4 has device validation on Pura 70 Pro+ / HBN-AL80, recorded in [the device validation notes](../VALIDATION_SHARED_JIEBA.md). ABI 1's Mate 90 Pro Max / OpenHarmony-7.0.0.109 record applies to the historical ABI 1 artifact. Host regression or an old device result must not be presented as validation of a new OHOS build.
- The generated application template declares compatible SDK 23 and target SDK 24. These are template settings, not proof of the native library's minimum supported OS/API. A minimum system version has not been established.
- Linux x86_64 and Windows x86_64 GNU are host-check targets. Linux CI does not certify OHOS execution. Windows tests may fail on file replacement when another process holds metadata files; the runner reports failures without retrying. MSVC and 32-bit layouts are not covered.

## Compatibility checks

`tools/check.py` uses these immutable source commits:

| Baseline | Source | Check |
| --- | --- | --- |
| Published ABI 1 | `29c0379fcf1f6df0db38a84979487d0fe3b03ba1` | Compile a legacy consumer with the old header/library, create its index, then execute the same binary with the candidate library |
| Published ABI 4 | `20851205341fb12cb44db89c0656fe9800042a6f` | Compile with the published header/library; preserve its binary and index while switching the runtime library |

The ABI 4 header's declarations and original byte hash are recorded in `tests/abi4-contract.json`. C structs, constants and existing prototypes cannot change unnoticed; additional declarations are allowed. Rust tests freeze the corresponding 64-bit C layout offsets. Old ABI 4 consumers also exercise APIs introduced in ABI 2/3, but they are not independently preserved binaries of those intermediate development stages.

Read-only checks compare the entire generated index file set, hashes, lengths and modification times before/after. These checks cover supported adapter-created indexes, not arbitrary indexes built with unrelated Tantivy versions or custom Rust schemas. History is used to reproduce fixtures; no user indexes are included or modified.
