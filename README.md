# tantivy-harmonyos

[简体中文](README.md) | [English](README_EN.md)

为 HarmonyOS arm64 应用提供 Tantivy 全文检索，通过 C 接口在 Native 模块中调用。支持 Jieba 中文分词、BM25、布尔与短语查询、字段排序、聚合、自定义词典，以及索引的写入、更新、删除和持久化。

本项目为非官方适配，使用 Tantivy **0.26.2** 和 jieba-rs **0.8.1**，未修改 Tantivy 上游源码。提供 C 头文件、动态库构建脚本和调用示例；ArkTS 应用需在自己的 Node-API 模块中调用 C 接口。

## 获取 SDK

通过下面的 Windows 交叉编译步骤获取配套的动态库和头文件，再复制到自己的工程。

Windows 构建需要 Git、[Rust](https://rustup.rs/)、DevEco 附带的 OpenHarmony Native SDK，以及 [llvm-mingw](https://github.com/mstorsjo/llvm-mingw/releases) 的 Windows x86_64 UCRT 工具链。已使用 Rust 1.90.0、llvm-mingw 20261006 UCRT 和 Native SDK API 26 构建；这不代表最低支持系统版本。

```powershell
git clone https://github.com/Torry2022/tantivy-harmonyos.git
cd tantivy-harmonyos
rustup toolchain install 1.90.0-x86_64-pc-windows-gnu --profile minimal
rustup target add aarch64-unknown-linux-ohos --toolchain 1.90.0-x86_64-pc-windows-gnu
$env:RUSTUP_TOOLCHAIN = '1.90.0-x86_64-pc-windows-gnu'
.\build_ohos.ps1 -OpenHarmonySdk 'C:\path\to\openharmony' `
  -HostLlvm 'C:\path\to\llvm-mingw' -Jobs 2
```

把两个示例路径替换为本机路径。`-OpenHarmonySdk` 指向包含 `native` 的 `openharmony` 目录；`-HostLlvm` 指向包含 `bin/llvm-dlltool.exe` 的解压目录。脚本按 `Cargo.lock` 构建，产物为：

```text
out/aarch64-unknown-linux-ohos/
├── libtantivy_ohos.so
└── tantivy_ohos.h
```

## 接入 HarmonyOS 工程

把库和头文件复制到自己的 Native 模块，例如：

```text
entry/src/main/
├── libs/arm64-v8a/libtantivy_ohos.so
└── cpp/
    ├── CMakeLists.txt
    └── third_party/tantivy/include/tantivy_ohos.h
```

在该模块的 `entry/src/main/cpp/CMakeLists.txt` 中添加以下配置。`entry` 是示例 Native 目标名，请替换成自己的 `add_library` 目标：

```cmake
add_library(tantivy_ohos SHARED IMPORTED)
set_target_properties(tantivy_ohos PROPERTIES
    IMPORTED_LOCATION "${CMAKE_CURRENT_SOURCE_DIR}/../libs/arm64-v8a/libtantivy_ohos.so"
    INTERFACE_INCLUDE_DIRECTORIES "${CMAKE_CURRENT_SOURCE_DIR}/third_party/tantivy/include")
target_link_libraries(entry PRIVATE tantivy_ohos)
```

在 C/C++ 中 `#include "tantivy_ohos.h"` 即可调用。Node-API 只需把 ArkTS 参数转为 UTF-8 路径和请求，再返回结果；建索引和查询应放到工作线程，避免阻塞 UI。同一句柄不能被多个任务同时调用。

索引保存在应用沙箱的可写目录，例如由 ArkTS 把 `context.filesDir + '/search-index'` 传给 Native。第一次用 `create`；以后启动用 `open`。父目录须已存在，创建时索引目录本身不能存在。

## 最小调用

下面是完整的 C99 函数：创建索引、写入文档、提交、查询并释放资源。`flags:15` 表示保存、索引、参与默认查询和默认返回。

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

重新打开已有索引时，替换创建调用：

```c
const tantivy_ohos_open_options options = {
    sizeof(options), TANTIVY_OHOS_READ_ONLY
};
tantivy_ohos_open(path, &options, NULL, &index, &error);
```

此处省略了返回码检查，实际代码须像上例一样检查。需要写入时使用 `TANTIVY_OHOS_READ_WRITE`。只读打开的 writer 参数必须为 `NULL`。

完整示例：[C99](examples/basic.c)、[C++11 RAII](examples/basic.cpp)、[排序、聚合与词典](examples/advanced.cpp)、[写入配置](examples/writer_options.c)。示例工程 [examples/CMakeLists.txt](examples/CMakeLists.txt) 可通过 `TANTIVY_INCLUDE_DIR` 和 `TANTIVY_LIBRARY` 指定匹配的头文件与库；交叉编译需另传 Native SDK toolchain，程序须在匹配平台运行。

## 字段与中文词典

`create` 接收下面这样的 JSON。`id` 是内置字段，不需声明，写入时单独传入。最多 64 个额外字段，字段名区分大小写，格式为 `[A-Za-z][A-Za-z0-9_]{0,63}`。

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

| kind | 类型 | 行为 |
| --- | --- | --- |
| `1` / `TANTIVY_OHOS_TEXT` | 文本 | Jieba 分词、小写转换，支持短语查询 |
| `2` / `TANTIVY_OHOS_KEYWORD` | 关键词 | 原样匹配，区分大小写 |
| `3` / `TANTIVY_OHOS_I64` | 整数 | 有符号 64 位 |
| `4` / `TANTIVY_OHOS_F64` | 小数 | 有限 double 值 |

`flags` 按位组合：`STORED=1`、`INDEXED=2`、`DEFAULT_QUERY=4`、`DEFAULT_RETURN=8`。字段至少保存或索引；默认返回要求保存，默认查询要求已索引的文本或关键词。`upsert` 使用 `tantivy_ohos_field_value` 传值；同一 ID 再写入会替换整条文档，未提供的字段变为缺失，不是局部更新。

排序和聚合字段要在 `fast_fields` 中声明，只支持 KEYWORD/I64/F64。词典配置可以省略；提供时扩充内置 Jieba 默认词典，词条与频率随索引持久化，重新打开无需外部词典文件。默认词频为 100000，允许 1..1000000000；最多 10000 个不重复词条，每个不超过 384 字节且不能含空白。字段与词典在创建后不可修改，调整时需新建索引并重新写入文档。

## 查询、分页与返回值

`search` 接收 JSON，返回结构化结果，不需要解析结果 JSON：

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

| 参数 | 含义与默认值 |
| --- | --- |
| `query` | 必填。支持 `AND`、`OR`、短语、字段和范围语法；词项间默认 AND，`*` 匹配全部文档 |
| `limit` | 必填，1..1000；`offset` 默认 0，二者之和不超过 10000 |
| `projection` | `default` 返回创建时的默认字段；`none` 只返回 ID/分数；`fields` 返回 `fields` 数组指定的已保存字段 |
| `query_fields` | 可选，覆盖不带字段名的查询默认字段；不会限制显式字段查询的访问范围 |
| `sort` | 省略时按 BM25；指定 fast field 后按 `asc`/`desc` 排序，分数为 0，缺失值始终在最后 |
| `track_total` | 默认 false；设为 true 才额外计算精确总命中数 |

用 `result_count` 读取当前页长度，`result_hit` 读取 ID/分数/字段数，`result_value` 按字段序号读取值。字符串通过 `string_value`，整数通过 `i64_value`，小数通过 `f64_value` 访问；缺失字段不出现在返回值中。`result_total` 的 `exact=false` 表示未计算总数，此时 total=0 不代表零命中。字段排序值由 `result_sort_value` 读取，`present=false` 表示该文档缺失排序字段。

翻页使用各次查询时已加载的索引快照；更新、reload 或段合并后不保证跨页稳定。只取 ID 且索引启用了默认 `fast_id:true` 时，不解压文档存储；请求保存字段时仍需读取文档。

## 聚合

将下面的 JSON 传给 `aggregate`，得到 `{"aggregations": ...}` 字符串，使用 `free_string` 释放。聚合针对全部匹配文档，和分页大小无关。

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

`terms` 用于 KEYWORD fast field；`stats` 和 `range` 用于 I64/F64 fast field。支持 1..16 个平铺聚合，不支持嵌套。terms 的分段候选可能使桶计数近似，返回上游误差信息；stats 使用 f64，大整数可能舍入。range 为左闭右开，I64 边界须为 ±2^53 内的 JSON 整数。完整参数限制见[头文件](include/tantivy_ohos.h)。

## 写入资源与生命周期

创建或读写打开时，writer 参数传 `NULL` 使用一个写入线程、15000000 字节预算；也可指定：

```c
const tantivy_ohos_writer_options writer = {
    sizeof(writer), 2, 30000000
};
tantivy_ohos_create(path, config, &writer, &index, &error);
```

线程数为 1..8，总预算平均分给各线程，每线程至少 15000000 字节且小于 4293967295 字节。这只是写入工作区预算，不是进程内存上限；分词器、文档队列、reader 和合并线程另有开销。设置只作用于当前句柄，不写入索引；只读打开不创建 writer。

- 每次调用检查返回码：0 成功，1 失败；错误字符串由库分配，调用方用 `free_string` 释放。
- 写入、删除后显式 `commit` 才持久化；`close` 不提交。其他 reader 需 `reload` 才能看到新提交。
- 同一索引最多一个 writer，可同时有多个只读句柄。同一句柄的全部操作必须串行，包括查询与关闭。
- 搜索结果独立于句柄，关闭句柄后仍可读。结果中的字符串是借用视图，仅在 `free_result` 前有效；不能单独释放。
- 每个结果、句柄及库分配的字符串恰好释放一次；三个释放函数均接受 `NULL`。

目前未提供多值字段、JSON/日期/字节字段、复杂嵌套聚合及稳定分页游标。完整函数声明和所有权约定以[头文件](include/tantivy_ohos.h)为准；开发与检查方法见 [CONTRIBUTING.md](CONTRIBUTING.md)。

## 许可

本仓库封装代码采用 [MIT](LICENSE)。Tantivy、jieba-rs、内置词典及其他依赖保留各自许可，见 [NOTICE](NOTICE) 和 [third_party](third_party)。分发二进制时应同时保留 SDK 附带的许可证与依赖声明。
