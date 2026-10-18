# tantivy-harmonyos

[简体中文](README.md) | [English](README_EN.md)

为 HarmonyOS arm64 应用提供 [Tantivy](https://github.com/quickwit-oss/tantivy) 全文检索，通过稳定的 C 调用约定供 Native 模块使用。支持 Jieba 中文分词、BM25 排序、布尔与短语查询，以及索引的写入、更新、删除和持久化。

本项目是非官方适配，使用官方 Tantivy **0.26.2** 和 jieba-rs **0.8.1**，没有修改 Tantivy 上游源码。当前提供 **ABI 4**，保留 ABI 1/2/3 接口。适配范围不等于 Tantivy 的全部 Rust API。

在 [Releases](https://github.com/Torry2022/tantivy-harmonyos/releases) 下载 `v0.26.2-ohos.2` 的 `tantivy-0.26.2-harmonyos-arm64-v8a.zip`，并用同页的 `SHA256SUMS` 校验。包内包含动态库、匹配的头文件、源码、示例和依赖许可。库位于 `lib/arm64-v8a/`，头文件位于 `include/`，完整源码和构建脚本位于 `source/`。旧 `ohos.1` 包仍是 ABI 1，不含新增接口。

## 接入

将 `libtantivy_ohos.so` 放入 `entry/src/main/libs/arm64-v8a/`，将 [tantivy_ohos.h](include/tantivy_ohos.h) 加入头文件搜索路径。在 CMake 中链接（假设调用方目标为 `entry`）：

```cmake
add_library(tantivy_ohos SHARED IMPORTED)
set_target_properties(tantivy_ohos PROPERTIES
    IMPORTED_LOCATION "${CMAKE_CURRENT_LIST_DIR}/../libs/arm64-v8a/libtantivy_ohos.so")
target_link_libraries(entry PRIVATE tantivy_ohos)
```

索引放在应用沙箱中。从 ArkTS 获取 `filesDir` 后传给 Native 模块。创建要求目录尚不存在，父目录已存在；已有目录不会被覆盖。失败的创建可能留下部分文件，请检查后改用新目录。

| 用途 | 接口 |
| --- | --- |
| 创建自定义字段索引并取得 writer | `tantivy_ohos_create_v2` |
| 打开已有索引读写 | `tantivy_ohos_open_v2`，`READ_WRITE` |
| 打开已有索引只读 | `tantivy_ohos_open_v2`，`READ_ONLY` |
| 替换整条文档 / 删除 ID | `tantivy_ohos_upsert_v2` / `tantivy_ohos_delete` |
| 持久化并刷新本句柄 | `tantivy_ohos_commit` |
| 刷新其他 reader 的已提交快照 | `tantivy_ohos_reload` |
| 查询并逐项读取结果 | `search_v2`、`result_count_v2`、`result_hit_v2`、`result_value_v2` |

完整、可编译的最小示例见 [C](examples/basic.c) 和 [C++](examples/basic.cpp)。示例的参数是新的索引目录，不会删除已有索引。

## 字段和查询

每个索引都有内置的 `id`：非空、区分大小写的精确字符串，始终建立索引并保存。可配置最多 64 个额外的单值字段，字段名为 `[A-Za-z][A-Za-z0-9_]{0,63}`，不能重复或使用 `id`。

| 类型 | 索引方式 | 用途 |
| --- | --- | --- |
| `TEXT` | Jieba 默认词典、Default 模式、HMM，随后转小写；保存位置 | 中文及混合文本、短语 |
| `KEYWORD` | 原样精确字符串，区分大小写 | 分类、标签、外部标识 |
| `I64` | 有符号 64 位整数 | 年份、计数、字段限定范围查询 |
| `F64` | 有限 double，不接受 NaN / Infinity | 数值属性、字段限定范围查询 |

字段分别选择 `STORED`、`INDEXED`、`DEFAULT_QUERY` 和 `DEFAULT_RETURN`。至少需要保存或索引之一；默认返回字段必须保存；默认查询字段必须是已索引的文本或精确字符串。存储选项决定能否返回原值，索引选项决定能否参与查询。索引创建后不能修改 schema。

`upsert_v2` 使用带类型的 C 值数组，按 ID 替换整条文档；未提供的字段会消失，空字符串则保留。字段重复、类型错误、未知字段或无效 UTF-8 会在提交任何修改之前报错。原始 `id/text` 配置仍要求提供 `text`（允许空字符串），保证旧 JSON 查询可用。

查询使用 Tantivy QueryParser：空格默认 AND，支持显式布尔运算、双引号短语及 `title:检索`、`year:[2020 TO 2026]` 等字段限定语法。`query_fields` 可覆盖一次查询中未限定字段的默认搜索范围；它不是权限过滤，显式字段语法仍可查询任何已索引字段。没有默认查询字段时请使用字段限定查询或 `*`。

配置随索引保存在 `tantivy-ohos.json`，重开时与真实 schema 校验。请整体保留索引目录，不要手动修改元数据。`config_json_v2` 可读取当前配置。v2 使用内置的 `zh_jieba_v1` 默认分词配置；v3 的领域词条扩展见下文。

高级接口的完整最小示例（需要 ABI 4）见 [advanced.cpp](examples/advanced.cpp)；纯 C 的跨进程调用样例见 [paging_smoke.c](smoke/paging_smoke.c)。

## 领域词典与 fast fields

`tantivy_ohos_create_v3` 接收版本化 JSON 配置，字段定义沿用 v2 的 kind/flags；`fast_fields` 单独声明用于排序或聚合的 KEYWORD、I64、F64 字段，不改变已有索引。示例：

```json
{"version":3,"tokenizer":"zh_jieba_v3","fields":[
  {"name":"title","kind":1,"flags":15},
  {"name":"category","kind":2,"flags":3},
  {"name":"price","kind":4,"flags":3}],
 "fast_fields":["category","price"],
 "dictionary":{"mode":"extend_default","entries":[
   {"word":"鸿蒙星河检索","frequency":1000000000}]}}
```

领域词典**扩展内置默认词典**，不替换默认词典，也不能删除默认词条。最多 10,000 个唯一词条，每条最多 384 UTF-8 字节，不含空白或 NUL；词频为 1..1,000,000,000，省略时为 100,000。省略 dictionary 表示无扩展。实际词条、词频和用于检测意外损坏的指纹均保存在索引的 `tantivy-ohos.json` 中，不依赖外部词典文件。重开（含只读）会校验快照；缺失或损坏会失败。指纹不是防篡改认证。

各索引的词典互不污染，索引创建后不能在线改词典；修改需新建索引并重新写入文档。v3 句柄在进程内按完整词典配置共享只读 Jieba 实例，不同配置保持隔离。池只持有弱引用，最后使用者释放后实例可回收；多个 reader 的其他内存成本仍需按实际配置测量。v2 创建路径继续使用原默认词典和原 schema。

## 排序与常用聚合

`tantivy_ohos_search_v3` 接收不超过 64 KiB 的 JSON，返回可通过现有 v2 getter 逐项读取、用 `free_result_v2` 释放的结构化结果：

```json
{"version":3,"query":"检索","limit":10,"offset":0,"projection":"none",
 "sort":{"field":"price","order":"asc"}}
```

省略 sort 按相关性排序；指定字段时用 Tantivy 原生 fast-field TopDocs 对完整命中集合选取结果，并非重排已截断的 TopK。支持 I64/F64/KEYWORD 升降序，缺失值在两方向均放最后；字符串按原样 UTF-8 字典序，不按语言区域排序。同值按升序 DocAddress 次排序，不支持额外的用户次排序键。I64 排序保留完整 64 位整数精度。

相关性排序的 score 是 BM25；字段排序不计算 BM25，score 固定为 0。通过 `result_sort_value_v3` 获取带类型的排序值及 present 标志，缺失时 present=false。limit 为 1..1000，offset+limit 不超过 10,000；资源随 offset+limit 增长。分页基于当前加载快照，不保证跨 reload、合并或更新的稳定游标。未配置 fast field 的旧索引会明确报错，不自动升级。空投影的 fast ID 路径与旧索引回退见下文。

`tantivy_ohos_aggregate_v3` 使用官方 AggregationCollector 对**完整查询匹配集合**做统计，与返回页或 TopK 无关：

```json
{"version":3,"query":"检索","aggregations":{
  "categories":{"terms":{"field":"category","size":10,"segment_size":100}},
  "prices":{"stats":{"field":"price"}},
  "bands":{"range":{"field":"price","ranges":[
    {"to":0,"key":"negative"},{"from":10,"to":20,"key":"high"}]}}},
 "memory_limit_bytes":67108864,"bucket_limit":4096}
```

响应是 `{"version":3,"aggregations":...}` JSON，用 `free_string` 释放。仅支持 1..16 个顶层聚合，不支持嵌套或其他聚合种类。terms 要求 KEYWORD fast field；size 默认为 10、最大 1024，segment_size 默认为 min(size×10,4096)、范围为 size..4096。按计数降序选候选，跨段截断可能使计数及头部词项近似；响应保留 `doc_count_error_upper_bound` 和 `sum_other_doc_count`，不能对任意数据宣称精确。

stats 要求数值 fast field，返回有效值的 count/sum/min/max/avg，忽略缺失。继承上游 f64 计算，I64 超过 2^53 时可能舍入，不承诺精确整数求和；非有限统计结果报错。零命中时 count=0、sum=0、min/max/avg=null。

range 要求数值 fast field，支持最多 128 个不重叠范围，允许间隙和未排序输入；from 包含、to 不包含，省略表示无界。仅返回请求桶，保持请求顺序，未指定 key 时使用 range_0、range_1 等。上游不支持重叠范围。I64 端点必须是 [-2^53,2^53] 内的 JSON 整数，在转 f64 前校验；超界或小数形式端点报错。F64 端点按有限 binary64 解释。缺失值不进桶。

桶数上限为 1..4096（默认 4096）；上游内部补齐的间隙/外侧桶也占用此额度，但不返回给调用方。内存上限为 1..64 MiB（默认 64 MiB），使用官方估算聚合内存守卫，不是进程 RSS 上限。实际超过预算会返回错误。未知字段、字段类型不符、无 fast field、非法范围或超限均报错。仅保存但未索引的字段可以用于排序/聚合，只要创建时配置了 fast field。

## 返回字段和所有权

每个命中始终包含 `id` 和 `score`；相关性排序时为 BM25，字段排序时固定为 0（不计算 BM25）。实际返回分数若非有限（如 boost 或评分运算溢出），查询明确失败，不返回 Infinity 或 JSON null。投影使用独立模式，避免空指针表达多种含义：

| 投影模式 | fields 参数 | 附加返回值 |
| --- | --- | --- |
| `PROJECT_DEFAULT` | `NULL/0` | 创建时标为 `DEFAULT_RETURN` 的字段，可为空 |
| `PROJECT_NONE` | `NULL/0` | 无，仅 ID / 分数 |
| `PROJECT_FIELDS` | 非空字段名数组 | 按指定顺序返回已保存字段；缺失值略过 |

自 ABI 4 起，`create_v3` 新建索引默认 `fast_id:true`，将 `id` 同时保存为原样字符串 fast column。有效投影为空时，`search_v2` / `search_v3` 从该列的词项序号读取 ID，不调用 docstore 解压；返回任一 stored 字段时仍解码文档。可在创建 JSON 中设 `fast_id:false` 关闭。旧元数据缺此键时按 false 读取，包括既有 v3 索引；旧索引继续从 docstore 取 ID，不会改写 schema 或元数据。v2 创建保持原 schema。没有把所有结果强制转换成 JSON；命中和字段通过带类型的只读 view 逐项访问。

所有操作返回 `0` 成功、`1` 错误。可选的 `char** error` 返回 UTF-8 错误消息。所有非空输出槽都会先清空，复用槽位前必须释放旧值。

- `open/create` 返回的句柄用 `tantivy_ohos_close` 释放。
- `search_v2` / `search_v3` 返回的结果用 `tantivy_ohos_free_result_v2` 释放，包括零命中结果。
- 错误字符串、旧查询 JSON、配置 JSON 和聚合 JSON 用 `tantivy_ohos_free_string` 释放。
- view 内字符串借用自结果，在结果释放前有效，不可单独释放。结果独立拥有数据，句柄关闭、commit 或 reload 后仍可使用。

输入是有效的 NUL 结尾 UTF-8，不支持内嵌 NUL。非空指针必须有效、对齐、长度正确且不与输出重叠。库不能识别任意失效指针或重复释放。可展开的 Rust panic 在 C 边界捕获；返回 panic 错误后应关闭该句柄；内存耗尽导致的进程终止不在此范围内。

v2 C options 结构的 `version` 设为 `2`、`struct_size` 设为对应类型的 `sizeof`；v3 JSON 的 `version` 设为 `3`。ABI 1/2 的布局和符号冻结；不认识的大小、版本、标志和模式会报错，将来的不兼容布局使用新符号。

## 提交、只读和线程

写入和删除在显式 `commit` 后持久化，commit 只刷新当前句柄。其他 reader 必须调用 `reload` 才看到新提交；查询使用各自最后加载的快照。`reload` 不提交，`close` 也不自动提交。如果提交后的 reader 刷新失败，数据可能已经持久化，请先 reload 或重开确认。

一个索引最多有一个 writer，可同时存在多个只读句柄。只读路径不创建 writer、不取得 writer 锁、不写入索引；写操作和 commit 明确报错。打开和 reload 时会使用现有 `.tantivy-meta.lock` 的只读文件句柄取得短暂元数据锁，以协调 writer 的文件回收；若锁文件缺失，只读操作会失败而不会创建它。请使用支持文件锁的本地文件系统。

同一句柄的全部调用（含查询、reload、close）必须串行。调用方同步后可在线程之间转移句柄；独立句柄可并发。结果的访问和释放也须串行，不能在使用 view 时释放结果。

## 兼容与范围

旧 `open/upsert/delete/commit/search/close/free_string` 函数签名保留。旧 `open` 仍是读写打开，创建时仍使用原来的 `id/text` 配置；原始索引即使没有适配器元数据文件也可重开。旧查询仍返回 `id/text/score` JSON，旧 upsert 和 search 拒绝自定义配置。新增只读打开可读取原始索引，也可继续使用旧 JSON 查询。`tantivy_ohos_abi_version()` 返回 4，累积包含 ABI 1/2/3 符号；调用方接受大于等于所需 ABI 的版本，v2 options 的 version 仍为 2。

查询 limit 为 1..1000；相关性排序返回分数最高的命中，不承诺同分顺序，字段排序的同值顺序见上文。`result_count_v2` 始终只返回页长度。需要总命中数时，在 `search_v3` JSON 中加 `track_total:true`，通过 `result_total_v4` 读取：exact=true 表示全部匹配文档的精确计数，包含零命中和 offset 超过末页；exact=false、total=0 表示未统计。默认不运行 Count 收集器，所有 search_v2 结果也标为未统计。页保持相同的 TopDocs 评分路径；显式请求总数时在同一 Searcher 快照上另行 Count 扫描，不将页长度当总数；不同分页调用之间的 reload 仍可能改变快照。尚未暴露多值字段、JSON/日期/字节字段、自定义次排序、复杂嵌套聚合、分页游标、查询对象构造、替换默认词典、索引迁移等能力。不要仅凭本适配器的接口范围推断鸿蒙或 Tantivy 的能力限制。

## 从源码构建

需要 Windows、Rust **1.90.0 GNU**、64 位 MinGW 工具和 DevEco OpenHarmony Native SDK。测试使用 llvm-mingw 20261006 UCRT x86_64。保持 `Cargo.lock`，不升级依赖：

```powershell
rustup toolchain install 1.90.0-x86_64-pc-windows-gnu --profile minimal
rustup target add aarch64-unknown-linux-ohos --toolchain 1.90.0-x86_64-pc-windows-gnu
$env:RUSTUP_TOOLCHAIN = '1.90.0-x86_64-pc-windows-gnu'
.\build_ohos.ps1 `
  -OpenHarmonySdk 'D:\path\to\DevEco Studio\sdk\default\openharmony' `
  -HostLlvm 'D:\path\to\llvm-mingw-20261006-ucrt-x86_64'
```

脚本按锁文件构建，默认两个并行任务；输出库和头文件位于 `out/aarch64-unknown-linux-ohos/`，SONAME 为 `libtantivy_ohos.so`。宿主测试用 `cargo test --locked --release -j 2`，需把 GNU 工具链链接工具加入 PATH。C/C++ 示例可用同平台编译器加 `-Iinclude`，并链接对应平台的 `tantivy_ohos` 动态库。

实际命令、结果与验证范围见[词典共享与真机验证](VALIDATION_SHARED_JIEBA.md)、[ABI 4 补充验证](VALIDATION_API_V4_SUPPLEMENT.md)、[早期 ABI 4 验证](VALIDATION_API_V4.md)、[公共 SDK 中文基准说明](benchmarks/README.md)、[v3 能力验证](VALIDATION_API_V3.md) 和 [ABI 2 验证记录](VALIDATION_API_V2.md)。[历史 ABI 1 记录](VALIDATION.md) 中的真机结果仅适用于当时的产物，不能代替后续 ABI 的设备验证。

Windows 宿主测试中存在间歇性的索引提交错误（系统错误 5），官方 Tantivy 原生调用也能复现同类错误，根因尚未确定。

ABI 3/4 新增接口的真机功能检查，以及性能与内存测量的适用范围，见[验证记录](VALIDATION_SHARED_JIEBA.md)。

需要在应用中验证新接口时，运行 `./prepare_api_smoke.ps1`，在 DevEco 打开生成的 `build/api-v4-harmony`。它使用独立包 `com.torry2022.tantivyapiprobe`，需配置匹配包名和目标设备的调试 Profile，可关联已有调试证书和密钥。生成脚本拒绝覆盖已有工程。v4 的 create/reopen/readonly 阶段调用见验证记录；原 v2 测试工程可用 `-ApiVersion 2 -Destination <新目录>` 生成。

## 许可与打包

适配代码和脚本采用 [MIT](LICENSE)。依赖保留原有许可，见 [NOTICE](NOTICE) 和 [third_party/](third_party/)。再分发需保留依赖、Rust 标准库和词典声明。

维护者在已审阅、干净的源码树中构建后，可运行 `python package_release.py --expected-library-sha256 <已核对的库哈希>` 生成本地归档。脚本记录源码提交、库哈希、ABI 和锁定依赖。打包不会发布或推送。
