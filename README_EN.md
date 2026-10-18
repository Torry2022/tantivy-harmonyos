# tantivy-harmonyos

[简体中文](README.md) | [English](README_EN.md)

[Tantivy](https://github.com/quickwit-oss/tantivy) full-text search for HarmonyOS arm64 applications through a C API for Native modules. Supports Jieba Chinese tokenization, BM25 ranking, Boolean and phrase queries, and persistent inserts, updates and deletes.

This unofficial adapter uses official Tantivy **0.26.2** and jieba-rs **0.8.1**, with no changes to Tantivy's upstream source. It provides **ABI 4** while retaining the ABI 1/2/3 interfaces. This adapter exposes a subset of Tantivy's Rust API.

Download `tantivy-0.26.2-harmonyos-arm64-v8a.zip` from the `v0.26.2-ohos.2` [release](https://github.com/Torry2022/tantivy-harmonyos/releases) and verify it against the accompanying `SHA256SUMS`. The archive contains the shared library, matching header, source, examples and dependency notices. The library is in `lib/arm64-v8a/`, the header in `include/`, and the complete source and build scripts in `source/`. The older `ohos.1` archive remains ABI 1 and does not include the new interfaces.

## Integration

Put `libtantivy_ohos.so` in `entry/src/main/libs/arm64-v8a/` and add [tantivy_ohos.h](include/tantivy_ohos.h) to your include path. Link it with CMake (assuming the calling target is `entry`):

```cmake
add_library(tantivy_ohos SHARED IMPORTED)
set_target_properties(tantivy_ohos PROPERTIES
    IMPORTED_LOCATION "${CMAKE_CURRENT_LIST_DIR}/../libs/arm64-v8a/libtantivy_ohos.so")
target_link_libraries(entry PRIVATE tantivy_ohos)
```

Keep indexes in the application sandbox. Obtain `filesDir` in ArkTS and pass it to Native code. Creation requires a new directory path with an existing parent; existing directories are never overwritten. A failed create may leave partial files: inspect them and use a new path.

| Purpose | API |
| --- | --- |
| Create a configurable index with a writer | `tantivy_ohos_create_v2` |
| Open an existing index read/write | `tantivy_ohos_open_v2`, `READ_WRITE` |
| Open an existing index read-only | `tantivy_ohos_open_v2`, `READ_ONLY` |
| Replace a whole document / delete by ID | `tantivy_ohos_upsert_v2` / `tantivy_ohos_delete` |
| Persist writes and refresh this handle | `tantivy_ohos_commit` |
| Refresh another reader's committed snapshot | `tantivy_ohos_reload` |
| Search and iterate typed results | `search_v2`, `result_count_v2`, `result_hit_v2`, `result_value_v2` |

See the complete compilable [C](examples/basic.c) and [C++](examples/basic.cpp) examples. Each takes a new index path and never deletes an existing index.

## Fields and queries

Every index has an implicit `id`: a nonempty, case-sensitive exact string, always stored and indexed. Configure up to 64 additional single-valued fields with unique case-sensitive names matching `[A-Za-z][A-Za-z0-9_]{0,63}`; `id` is reserved.

| Type | Indexing | Use |
| --- | --- | --- |
| `TEXT` | Jieba default dictionary/mode, HMM, lowercase, positions | Chinese and mixed text, phrases |
| `KEYWORD` | Raw case-sensitive whole string | Categories, tags, external identifiers |
| `I64` | Signed 64-bit integer | Years, counts, field-qualified range queries |
| `F64` | Finite double; NaN and infinity rejected | Numeric attributes and field-qualified ranges |

Select `STORED`, `INDEXED`, `DEFAULT_QUERY` and `DEFAULT_RETURN` independently. A field must be stored and/or indexed. Default return fields must be stored; default query fields must be indexed text or keyword fields. Storage controls whether the original value can be returned; indexing controls whether it can be queried. The schema is immutable after creation.

`upsert_v2` takes a typed C array and replaces the entire document by ID. Omitted fields disappear; empty strings remain present. Duplicate fields, wrong types, unknown fields and invalid UTF-8 fail before any mutation is queued. The original `id/text` configuration still requires `text` (possibly empty), including when written with v2, preserving legacy JSON search compatibility.

Queries use Tantivy QueryParser: implicit AND, explicit Boolean operators, quoted phrases and field-qualified expressions such as `title:检索` or `year:[2020 TO 2026]`. `query_fields` overrides default fields for unqualified terms for one query. It is not an access filter: explicit field syntax can target any indexed field. With no default query fields, use field-qualified syntax or `*`.

Configuration is saved in `tantivy-ohos.json` beside the index and checked against the actual schema on reopen. Keep the whole directory together and do not edit metadata manually. Read current configuration with `config_json_v2`. The v2 path uses the built-in `zh_jieba_v1` default tokenizer; v3 domain extensions are described below.

See [advanced.cpp](examples/advanced.cpp) for a minimal advanced example (requires ABI 4) and [paging_smoke.c](smoke/paging_smoke.c) for a C consumer with separate-process phases.

## Domain dictionaries and fast fields

`tantivy_ohos_create_v3` accepts versioned JSON. Field kind/flags retain their v2 meanings; a separate `fast_fields` list enables sorting/aggregation on KEYWORD, I64 or F64 fields in a new index:

```json
{"version":3,"tokenizer":"zh_jieba_v3","fields":[
  {"name":"title","kind":1,"flags":15},
  {"name":"category","kind":2,"flags":3},
  {"name":"price","kind":4,"flags":3}],
 "fast_fields":["category","price"],
 "dictionary":{"mode":"extend_default","entries":[
   {"word":"鸿蒙星河检索","frequency":1000000000}]}}
```

Domain entries **extend the embedded default dictionary**; they neither replace it nor remove default words. At most 10,000 unique words, each <=384 UTF-8 bytes without whitespace/NUL. Frequency is 1..1,000,000,000, default 100,000. An omitted dictionary means no extensions. Actual entries/frequencies and an accidental-corruption fingerprint are saved in the index's `tantivy-ohos.json`, without an external dictionary-file dependency. Reopen, including read-only open, validates the snapshot and rejects missing/corrupt data. The fingerprint is not authentication.

Dictionaries are isolated between indexes and immutable after creation; changes require a new index and reindexing. v3 handles in the same process share an immutable Jieba instance when their complete dictionary configurations match. Different configurations remain isolated. The pool holds only weak references, allowing the instance to be reclaimed after its last user releases it; other reader memory costs still depend on the workload. The v2 creation path retains its original default dictionary and schema.

## Sorting and common aggregations

`tantivy_ohos_search_v3` takes JSON <=64 KiB and returns a structured result accessed with existing v2 getters and released with `free_result_v2`:

```json
{"version":3,"query":"检索","limit":10,"offset":0,"projection":"none",
 "sort":{"field":"price","order":"asc"}}
```

Omit sort for relevance. Field sorting uses native fast-field TopDocs over all query matches, rather than reordering a truncated TopK. I64/F64/KEYWORD support asc/desc; missing values are last in either direction. Strings use raw UTF-8 lexicographic order, not locale collation. Ties use ascending DocAddress, with no configurable secondary key. I64 sorting preserves all 64 integer bits.

Relevance score is BM25. Field sorting does not compute BM25 and reports score=0. `result_sort_value_v3` returns the typed sort value and a present flag; missing values have present=false. Limit is 1..1000 and offset+limit <=10,000; resource use grows with offset+limit. Pages use the loaded snapshot and are not stable cursors across reloads, merges or updates. Old indexes without the required fast field return an error, without migration. See below for fast ID access with empty projection and the fallback for old indexes.

`tantivy_ohos_aggregate_v3` uses the official AggregationCollector over the **entire query match set**, independently of the result page or TopK:

```json
{"version":3,"query":"检索","aggregations":{
  "categories":{"terms":{"field":"category","size":10,"segment_size":100}},
  "prices":{"stats":{"field":"price"}},
  "bands":{"range":{"field":"price","ranges":[
    {"to":0,"key":"negative"},{"from":10,"to":20,"key":"high"}]}}},
 "memory_limit_bytes":67108864,"bucket_limit":4096}
```

The response is `{"version":3,"aggregations":...}` JSON, freed with `free_string`. Only 1..16 flat top-level aggregations are supported. Terms requires a KEYWORD fast field. Size defaults to 10, maximum 1024; segment_size defaults to min(size×10,4096), allowed size..4096. Count-descending candidates can be truncated per segment, so counts/top terms may be approximate. The response includes `doc_count_error_upper_bound` and `sum_other_doc_count`; do not assume exactness on arbitrary data.

Stats requires a numeric fast field and returns count/sum/min/max/avg over present values. Native f64 arithmetic can round I64 values above 2^53; exact integer sums are not promised. Non-finite statistics return an error. Zero matches yield count=0, sum=0 and null min/max/avg.

Range requires a numeric fast field and accepts up to 128 nonoverlapping ranges, including gaps and unsorted input. From is inclusive, to exclusive; omission is unbounded. Only requested buckets are returned in request order; default keys are range_0, range_1, etc. Upstream does not support overlapping ranges. I64 endpoints must be JSON integers in [-2^53,2^53], checked before conversion to f64; larger or decimal-form endpoints fail. F64 endpoints use finite binary64 values. Missing values do not enter buckets.

Bucket limit is 1..4096 (default 4096), including native gap/outer buckets before they are filtered from the response. Memory limit is 1..64 MiB (default 64 MiB), using the native estimated aggregation-memory guard, not a process RSS cap. Actual budget violations return errors. Unknown/wrong-type/non-fast fields and invalid ranges/options fail explicitly. Stored-only fields can be sorted/aggregated when configured as fast fields at creation.

## Projection and ownership

Every hit includes `id` and `score`: BM25 for relevance sorting, or 0 (BM25 not computed) for field sorting. A non-finite returned score, including boost or arithmetic overflow, fails the query rather than returning infinity or JSON null. Select projection through a dedicated mode:

| Projection | fields argument | Additional returned values |
| --- | --- | --- |
| `PROJECT_DEFAULT` | `NULL/0` | Saved `DEFAULT_RETURN` fields; may be empty |
| `PROJECT_NONE` | `NULL/0` | None; ID and score only |
| `PROJECT_FIELDS` | Nonempty names array | Named stored fields in requested order; missing values omitted |

Since ABI 4, new `create_v3` indexes default to `fast_id:true`, adding a raw string fast column to `id`. When the effective projection is empty, `search_v2` / `search_v3` resolve ID ordinals through that column without calling docstore decompression. Returning any stored field still decodes the document. Set `fast_id:false` in creation JSON to opt out. Missing metadata means false when opening an old index, including existing v3 indexes: these use stored-ID fallback without schema or metadata writes. v2 creation retains its original schema. Callers can iterate typed hit/value views without parsing a JSON result array.

Operations return `0` on success and `1` on error. The optional `char** error` receives an owned UTF-8 message. Non-NULL output slots are cleared before use; release previous allocations before reusing a slot.

- Release handles from open/create with `tantivy_ohos_close`.
- Release search_v2 / search_v3 results with `tantivy_ohos_free_result_v2`, including zero-hit results.
- Release error messages, legacy search JSON, configuration JSON and aggregation JSON with `tantivy_ohos_free_string`.
- Strings in views borrow from the result and remain valid until that result is freed. Never free them separately. Results own their data and survive handle close, commit and reload.

Inputs must be valid NUL-terminated UTF-8 without embedded NUL. Non-NULL pointers must be live, aligned, correctly sized and must not alias outputs. Arbitrary dangling pointers and double frees cannot be detected. Unwinding Rust panics are caught at C boundaries; close a handle after a panic error. Process aborts, including allocation failure, are outside this guarantee.

Set v2 C options `version` to `2` and `struct_size` to `sizeof` its exact type; set v3 JSON `version` to `3`. ABI 1/2 layouts and symbols remain frozen. Unknown sizes, versions, flags and modes fail; future incompatible layouts use new symbols.

## Commit, read-only access and threads

Writes and deletes become durable on explicit commit, which refreshes only that handle's reader. Other readers call `reload` to see a new commit; searches use their last loaded snapshot. Reload does not commit, and close never commits. If refresh fails after a commit, data may already be durable: reload or reopen before retrying.

One writer and multiple read-only handles may coexist. Read-only access creates no writer, takes no writer lock and writes no index files. Mutation and commit calls return clear errors. Opening/reloading briefly takes the existing `.tantivy-meta.lock` using read-only file access to coordinate with writer garbage collection. If that file is missing, the read-only operation fails without creating it. Use a local filesystem with OS file-lock support.

Serialize every operation on a handle, including search, reload and close. Handles may move between threads with caller synchronization; separate handles may run concurrently. Also serialize result access and release: do not free a result while a view is in use.

## Compatibility and scope

All legacy `open/upsert/delete/commit/search/close/free_string` signatures remain available. Legacy open still opens a writer and creates the original `id/text` configuration. Original indexes without adapter metadata are recognized automatically. Legacy search still returns `id/text/score` JSON; legacy upsert and search reject custom configurations. A v2 read-only handle can open an original index and use legacy JSON search. `tantivy_ohos_abi_version()` returns 4, cumulatively including ABI 1/2/3 symbols. Accept versions >= the required ABI; v2 options retain version=2.

Search limits are 1..1000. Relevance sorting returns the highest-scoring hits without promising tie order; field-sort ties follow the rule above. `result_count_v2` always returns page length only. To request a total, add `track_total:true` to `search_v3` JSON and read `result_total_v4`: exact=true means the precise number of all matching live documents, including zero matches and offsets beyond the last hit. exact=false with total=0 means not counted. By default no Count collector runs; all search_v2 results are also marked not counted. The page always uses the same TopDocs scoring path. An explicit total request adds a separate Count scan on the same Searcher snapshot. Separate page calls may still see different snapshots after reload. Multi-valued fields, JSON/date/bytes fields, custom secondary sorts, complex nested aggregations, pagination cursors, query-object construction, replacement of the default dictionary and index migration are not exposed. The adapter's scope does not define the capabilities of HarmonyOS or Tantivy.

## Build from source

Requires Windows, Rust **1.90.0 GNU**, 64-bit MinGW tools and the DevEco OpenHarmony Native SDK. The tested MinGW distribution is llvm-mingw 20261006 UCRT x86_64. Keep `Cargo.lock` and the pinned dependencies:

```powershell
rustup toolchain install 1.90.0-x86_64-pc-windows-gnu --profile minimal
rustup target add aarch64-unknown-linux-ohos --toolchain 1.90.0-x86_64-pc-windows-gnu
$env:RUSTUP_TOOLCHAIN = '1.90.0-x86_64-pc-windows-gnu'
.\build_ohos.ps1 `
  -OpenHarmonySdk 'D:\path\to\DevEco Studio\sdk\default\openharmony' `
  -HostLlvm 'D:\path\to\llvm-mingw-20261006-ucrt-x86_64'
```

The script uses the lockfile and two parallel jobs by default. It writes a matching library and header to `out/aarch64-unknown-linux-ohos/`, with SONAME `libtantivy_ohos.so`. Run host tests with `cargo test --locked --release -j 2`, with GNU linker tools on PATH. Compile the C/C++ examples with a compiler for the same platform, `-Iinclude`, and the corresponding `tantivy_ohos` shared library.

See [dictionary sharing and hardware validation](VALIDATION_SHARED_JIEBA.md), [ABI 4 supplementary validation](VALIDATION_API_V4_SUPPLEMENT.md), [earlier ABI 4 validation](VALIDATION_API_V4.md), the [public SDK Chinese benchmark protocol](benchmarks/README.md), [v3 capability validation](VALIDATION_API_V3.md) and [ABI 2 validation](VALIDATION_API_V2.md) for actual commands, results and remaining checks. [Historical ABI 1 validation](VALIDATION.md) applies only to its original artifacts and is not device validation of later ABIs.

Windows host tests have encountered intermittent index commit failures (system error 5). Native calls to official Tantivy can also reproduce the same class of error; the root cause remains unknown.

See the [validation record](VALIDATION_SHARED_JIEBA.md) for hardware functional checks of the interfaces added in ABI 3/4 and the scope of performance and memory measurements.

To validate the new API inside an application, run `./prepare_api_smoke.ps1` and open the generated `build/api-v4-harmony` in DevEco. It uses the independent bundle `com.torry2022.tantivyapiprobe`. Configure a debug Profile matching that bundle and device; it can use an existing debug certificate and key. The generator refuses to overwrite an existing project. See the validation record for v4 create/reopen/readonly stages. Generate the original v2 test project with `-ApiVersion 2 -Destination <new-directory>`.

## License and packaging

The adapter and scripts use [MIT](LICENSE). Dependencies retain their own licenses; see [NOTICE](NOTICE) and [third_party/](third_party/). Preserve dependency, Rust standard-library and dictionary notices when redistributing.

After reviewing and building a clean source tree, maintainers can run `python package_release.py --expected-library-sha256 <reviewed-library-hash>` to create a local archive. It records the source commit, library hash, ABI and pinned dependencies. Packaging does not publish or push anything.
