# Interface and index support

The current SDK has one C interface, declared in `include/tantivy_ohos.h`. Previous numbered interfaces are retired: consumers must update their calls and rebuild. Use a matching library and header from the same source revision or release. No historical binary compatibility is promised.

The index metadata format is separate from the C interface. Existing supported metadata is still read and validated against the Tantivy schema. Missing `fast_id` uses stored IDs without rewriting the index. No automatic schema/dictionary migration occurs, and read-only access never creates or changes index files. Field or analyzer changes require a new index and reindexing. Before changing the upstream Tantivy version, check existing index fixtures and document any rebuild requirement.

The distribution target is `aarch64-unknown-linux-ohos`, with a shared library and C header. Pinned dependencies are Tantivy 0.26.2, jieba-rs 0.8.1 and Rust 1.90.0. The reference cross-build uses Windows GNU host tools, llvm-mingw 20261006 UCRT and OpenHarmony Native SDK API 26 (26.0.0.105). The sample application declares compatible SDK 23 and target SDK 24; those settings do not establish the native library's minimum supported system version.

Linux x86_64 and Windows x86_64 GNU are host-check targets. MSVC and 32-bit layouts are not covered. Host tests cannot certify HarmonyOS execution. Historical device notes under `history/` apply only to their recorded artifacts; current checks belong in [VALIDATION.md](VALIDATION.md).
