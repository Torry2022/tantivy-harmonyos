# tantivy-harmonyos

[简体中文](README.md) | [English](README_EN.md)

Tantivy full-text search for HarmonyOS arm64 applications, exposed through a C interface for Native modules. Supports Jieba Chinese tokenization, BM25, Boolean and phrase queries, field sorting, aggregations, dictionary extensions, and persistent index writes, updates and deletions.

This unofficial adapter uses Tantivy **0.26.2** and jieba-rs **0.8.1**, without modifying Tantivy's upstream source. It supplies a C header, shared-library build script and examples. ArkTS applications call the C interface through their own Node-API module.

## Get the SDK

Cross-compile the shared library and its matching header on Windows as follows, then copy them into your project.

Windows builds need Git, [Rust](https://rustup.rs/), the OpenHarmony Native SDK bundled with DevEco, and the Windows x86_64 UCRT toolchain from [llvm-mingw](https://github.com/mstorsjo/llvm-mingw/releases). Builds have used Rust 1.90.0, llvm-mingw 20261006 UCRT and Native SDK API 26; this does not establish a minimum supported OS version.

```powershell
git clone https://github.com/Torry2022/tantivy-harmonyos.git
cd tantivy-harmonyos
rustup toolchain install 1.90.0-x86_64-pc-windows-gnu --profile minimal
rustup target add aarch64-unknown-linux-ohos --toolchain 1.90.0-x86_64-pc-windows-gnu
$env:RUSTUP_TOOLCHAIN = '1.90.0-x86_64-pc-windows-gnu'
.\build_ohos.ps1 -OpenHarmonySdk 'C:\path\to\openharmony' `
  -HostLlvm 'C:\path\to\llvm-mingw' -Jobs 2
```

Replace the two example paths. `-OpenHarmonySdk` points to the `openharmony` directory containing `native`; `-HostLlvm` points to the extracted directory containing `bin/llvm-dlltool.exe`. The script builds with `Cargo.lock` and produces:

```text
out/aarch64-unknown-linux-ohos/
├── libtantivy_ohos.so
└── tantivy_ohos.h
```

## Integrate into a HarmonyOS project

Copy the library and header into your Native module, for example:

```text
entry/src/main/
├── libs/arm64-v8a/libtantivy_ohos.so
└── cpp/
    ├── CMakeLists.txt
    └── third_party/tantivy/include/tantivy_ohos.h
```

Add the following to that module's `entry/src/main/cpp/CMakeLists.txt`. Replace `entry` with your own `add_library` target:

```cmake
add_library(tantivy_ohos SHARED IMPORTED)
set_target_properties(tantivy_ohos PROPERTIES
    IMPORTED_LOCATION "${CMAKE_CURRENT_SOURCE_DIR}/../libs/arm64-v8a/libtantivy_ohos.so"
    INTERFACE_INCLUDE_DIRECTORIES "${CMAKE_CURRENT_SOURCE_DIR}/third_party/tantivy/include")
target_link_libraries(entry PRIVATE tantivy_ohos)
```

Include `tantivy_ohos.h` in C/C++. Your Node-API bridge converts ArkTS arguments to UTF-8 paths/requests and returns results. Perform indexing and searching on worker threads to avoid blocking the UI. Do not access one handle concurrently from multiple tasks.

Store indexes in a writable application sandbox directory, for example by passing `context.filesDir + '/search-index'` from ArkTS to Native. Use `create` once and `open` on subsequent launches. The parent must exist; the index directory itself must not exist when creating.

## Minimal call sequence

This complete C99 function creates an index, writes a document, commits, searches and releases resources. `flags:15` means stored, indexed, default query and default return.

```c
#include "tantivy_ohos.h"
#include <stdio.h>

/* path must not exist; its parent must exist. */
int index_example(const char* path) {
    tantivy_ohos_handle* index = NULL;
    tantivy_ohos_result* result = NULL;
    char* error = NULL;
    int status = 1;
    const char* config =
        "{\"fields\":[{\"name\":\"text\",\"kind\":1,\"flags\":15}]}";
    const tantivy_ohos_field_value text = {
        "text", TANTIVY_OHOS_TEXT, "鸿蒙设备支持本地中文检索", 0, 0.0
    };
    if (tantivy_ohos_create(path, config, NULL, &index, &error)) goto cleanup;
    if (tantivy_ohos_upsert(index, "doc-1", &text, 1, &error)) goto cleanup;
    if (tantivy_ohos_commit(index, &error)) goto cleanup;
    if (tantivy_ohos_search(index,
        "{\"query\":\"中文检索\",\"limit\":10,\"projection\":\"none\"}",
        &result, &error)) goto cleanup;
    size_t count = 0;
    if (tantivy_ohos_result_count(result, &count, &error)) goto cleanup;
    for (size_t i = 0; i < count; ++i) {
        tantivy_ohos_hit_view hit = {0};
        if (tantivy_ohos_result_hit(result, i, &hit, &error)) goto cleanup;
        printf("%s: %g\n", hit.id, (double)hit.score);
    }
    status = 0;
cleanup:
    if (error) fprintf(stderr, "%s\n", error);
    tantivy_ohos_free_string(error);
    tantivy_ohos_free_result(result);
    tantivy_ohos_close(index);
    return status;
}
```

To reopen an existing index, replace the create call with:

```c
const tantivy_ohos_open_options options = {
    sizeof(options), TANTIVY_OHOS_READ_ONLY
};
tantivy_ohos_open(path, &options, NULL, &index, &error);
```

Return-code handling is omitted in this short fragment; check it as in the complete example. Use `TANTIVY_OHOS_READ_WRITE` for writes. Read-only open requires a `NULL` writer argument.

Complete examples: [C99](examples/basic.c), [C++11 RAII](examples/basic.cpp), [sorting, aggregations and dictionaries](examples/advanced.cpp), and [writer settings](examples/writer_options.c). The [CMake example project](examples/CMakeLists.txt) accepts matching `TANTIVY_INCLUDE_DIR` and `TANTIVY_LIBRARY` paths. For cross-compilation, also supply the Native SDK toolchain; run the executables on the matching target platform.

## Fields and Chinese dictionaries

Pass a JSON configuration like this to `create`. `id` is built in: do not declare it; pass it separately on upsert. Up to 64 additional fields are supported, with unique case-sensitive names matching `[A-Za-z][A-Za-z0-9_]{0,63}`.

```json
{
  "fields": [
    {"name": "title", "kind": 1, "flags": 15},
    {"name": "category", "kind": 2, "flags": 3},
    {"name": "price", "kind": 4, "flags": 3}
  ],
  "fast_fields": ["category", "price"],
  "dictionary": {
    "mode": "extend_default",
    "entries": [{"word": "鸿蒙星河检索", "frequency": 1000000000}]
  }
}
```

| kind | Type | Behavior |
| --- | --- | --- |
| `1` / `TANTIVY_OHOS_TEXT` | Text | Jieba tokenization, lowercase, phrase positions |
| `2` / `TANTIVY_OHOS_KEYWORD` | Keyword | Exact, case-sensitive matching |
| `3` / `TANTIVY_OHOS_I64` | Integer | Signed 64-bit |
| `4` / `TANTIVY_OHOS_F64` | Number | Finite double |

Combine flag bits: `STORED=1`, `INDEXED=2`, `DEFAULT_QUERY=4`, `DEFAULT_RETURN=8`. A field must be stored or indexed. Default returns require storage; default queries require indexed text/keyword fields. Pass values through `tantivy_ohos_field_value`. Upserting an existing ID replaces the whole document: omitted fields become absent, rather than remaining unchanged.

Declare sorting and aggregation fields in `fast_fields`; only KEYWORD/I64/F64 are supported. The optional dictionary extends the embedded Jieba default dictionary. Entries and frequencies persist with the index, so no external dictionary is needed on reopen. Frequencies default to 100000 and range from 1 to 1000000000. At most 10000 unique entries are allowed, each up to 384 UTF-8 bytes without whitespace. Schema and dictionary settings are immutable: changes require a new index and reindexing.

## Queries, pagination and results

`search` accepts JSON and returns typed results, without requiring result JSON parsing:

```json
{
  "query": "category:book AND price:[10 TO 100]",
  "limit": 10,
  "offset": 0,
  "projection": "fields",
  "fields": ["title", "price"],
  "sort": {"field": "price", "order": "asc"},
  "track_total": true
}
```

| Option | Meaning and default |
| --- | --- |
| `query` | Required. Boolean, phrase, field and range syntax; implicit AND; `*` matches all documents |
| `limit` | Required, 1..1000; `offset` defaults to 0; their sum must not exceed 10000 |
| `projection` | `default` uses saved default-return fields; `none` returns ID/score only; `fields` uses the named stored fields in the `fields` array |
| `query_fields` | Optional unqualified-query default override; does not restrict explicitly field-qualified queries |
| `sort` | BM25 when omitted; otherwise a fast field with `asc`/`desc`, score=0 and missing values last |
| `track_total` | False by default; true requests an additional exact matching-document count |

Use `result_count` for page length, `result_hit` for ID/score/field count and `result_value` to iterate returned fields. Read strings from `string_value`, integers from `i64_value` and doubles from `f64_value`. Absent values are omitted. In `result_total`, `exact=false` means no count was requested: total=0 does not imply no matches. `result_sort_value` returns a field-sorting value; `present=false` means that document lacks it.

Pages use the snapshot loaded for each call; updates, reloads or segment merges can change later pages. ID-only searches with the default `fast_id:true` avoid document-store decompression. Returning any stored field still reads the document store.

## Aggregations

Pass JSON like this to `aggregate`. It returns an owned `{"aggregations": ...}` string to release with `free_string`. Aggregations cover all matching documents, independently of page size.

```json
{
  "query": "*",
  "aggregations": {
    "categories": {"terms": {"field": "category", "size": 10}},
    "prices": {"stats": {"field": "price"}},
    "bands": {"range": {"field": "price", "ranges": [{"to": 10}, {"from": 10}]}}
  }
}
```

`terms` needs a KEYWORD fast field; `stats` and `range` need I64/F64 fast fields. One to sixteen flat aggregations are supported, without nesting. Per-segment terms candidates can produce approximate bucket counts, with upstream error information returned. Stats use f64 and may round large integers. Ranges include the lower bound and exclude the upper; I64 boundaries must be JSON integers within ±2^53. See the [header](include/tantivy_ohos.h) for all parameter limits.

## Writer resources and lifetime

A `NULL` writer argument on create/read-write open selects one indexing worker and 15000000 bytes. To override it:

```c
const tantivy_ohos_writer_options writer = {
    sizeof(writer), 2, 30000000
};
tantivy_ohos_create(path, config, &writer, &index, &error);
```

Use 1..8 workers. The total budget is divided between them, with at least 15000000 and less than 4293967295 bytes per worker. This is an indexing arena budget, not a process memory cap: tokenizers, document queues, readers and merge workers cost extra. Settings apply only to that handle and are not saved in index metadata. Read-only open creates no writer.

- Check every return code: 0 succeeds, 1 fails. Release library-allocated errors with `free_string`.
- Writes and deletes become durable only on explicit `commit`; `close` does not commit. Other readers call `reload` to see new commits.
- One writer per index, alongside multiple readers. Serialize every operation on a handle, including search and close.
- Results are independent of handles and remain readable after handle close. Their string views are borrowed until `free_result`; never free those strings separately.
- Release every result, handle and owned string exactly once. All three release functions accept `NULL`.

Multi-valued fields, JSON/date/byte fields, complex nested aggregations and stable pagination cursors are not exposed. The [header](include/tantivy_ohos.h) defines the complete interface and ownership rules. Development and check instructions are in [CONTRIBUTING.md](CONTRIBUTING.md).

## License

The adapter code is [MIT licensed](LICENSE). Tantivy, jieba-rs, embedded dictionary data and other dependencies retain their own licenses; see [NOTICE](NOTICE) and [third_party](third_party). Preserve SDK license files and dependency notices when redistributing binaries.
