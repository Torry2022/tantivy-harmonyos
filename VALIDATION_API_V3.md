# v3 能力批次 A 验证记录

2026-10-17。本批为领域词典、字段排序与 terms/stats/range 聚合；高效 fast ID、总匹配数及最终资源基准属于后续 B，本记录不宣称整个 SDK 已完成。独立克隆、分支和发布边界沿用 [ABI 2 记录](VALIDATION_API_V2.md)：`codex/public-c-api`，未提交、推送或发布。

## 固定依赖和上游依据

Tantivy 0.26.2、jieba-rs 0.8.1、Rust 1.90.0；`Cargo.toml`、`Cargo.lock` 和工具链文件未改变，无上游补丁。实现前直接检查锁定包源码：

- jieba-rs `src/lib.rs` 的 `Jieba::new` / `add_word`：扩展内置默认词典，词频和词条在每个索引的不可变实例中生效。
- Tantivy `collector/top_score_collector.rs` 的 `order_by_fast_field` / `order_by_string_fast_field` / `and_offset`，`collector/sort_key/order.rs` 的缺失值比较器：原生 TopDocs 在完整命中集合取页，升降序均缺失最后，同值 DocAddress 升序。
- `aggregation/collector.rs` 的 AggregationCollector，`aggregation/agg_limits.rs` 的估算内存与返回桶守卫，`aggregation/bucket/term_agg.rs` 的按段候选截断。
- `aggregation/bucket/range.rs` 的 `extend_validate_ranges`：上游自动补齐间隙及外侧桶，不支持重叠。本封装允许间隙和未排序输入，仍由官方 collector 收集，通过内部分配的桶键仅选回请求桶，保持请求顺序。

v3 JSON 接口是新增符号，冻结的 v2 C 结构布局不变。ABI 版本返回 3，表示累积包含 ABI1/2；options_v2 仍为 version=2。要求精确等于旧 abi_version 的调用方需接受新能力版本；没有修改或删除旧符号。

## 宿主 Rust 验证

```powershell
# 同 ABI 2 记录中的本地 GNU 环境
cargo fmt -- --check
cargo test --locked --release -j 2 -- --nocapture
cargo build --locked --release --lib -j 2
```

14 项通过，保留此前 10 项有效回归。`--nocapture` 日志中的 `test panic` 来自专门验证 boundary 捕获的既有测试，该项通过；不是吞掉测试失败。

新增断言与实际结果：

- 同时创建默认、扩展“鸿蒙星河检索”、扩展“端侧向量检索”的三个索引。相同查询默认/另一词典命中 a、b，目标词典仅命中整词文档 a；外部输入文件删除后仍能关闭、只读重开并复现，重开读写后的新文档也使用同词典。保存快照缺失、词条被改、模式被改或元数据缺失时明确失败，失败只读打开前后文件字节不变。
- 最初对照正文插入了额外词项，默认查询未命中。直接用锁定 Jieba 输出核对后，确认 Tantivy 对单个未分隔词项分出的多个 token 使用短语语义；将对照正文改为同 token 的空格形式，保持“默认命中两篇、自定义仅一篇”的断言，没有降低为只检查参数被接受。
- 75 文档、多于 8 个段，独立根据输入值和快照 DocAddress 算出预期。I64/F64/KEYWORD 六种排序逐页（每页 7 条）核对全序列、不重不漏、同值、缺失、空字符串与 Unicode；每种取回 75 条跨页结果。I64 的 MIN、MAX、2^53、2^53+1 及 F64 边界另行验证，返回值保持真实类型。
- 完整匹配集合的独立聚合预期：数值有效 count=68、sum=52、min=-37、max=36、avg=52/68；范围负数/0..10/10以上分别 28/20/20；terms 的 Z、空串、é、a、中分别 14/14/14/13/13。查询返回页仅 7 条，不影响完整集合统计。
- 有间隙且逆序请求的 I64/F64 范围仅返回所请求桶，保留顺序，端点 from 包含、to 不包含。零命中、缺失、负数均与独立预期核对。
- 原生内存预算 1 字节实际拒绝，报告当前估算约 1.25 KB；bucket_limit=1 实际拒绝，报告当前 8 个桶。请求的数值/请求大小/桶数/字段类型/缺少 fast field/非法范围亦有错误检查。
- 刻意构造三段候选截断：真实全局 X=12，但 segment_size=1 的 size=1 返回一个段头部词项 count=5，doc_count_error_upper_bound=12、sum_other_doc_count=22；增大候选后 X=12。证明响应诚实保留近似误差，不把任意候选设置说成精确统计。
- I64 stats 继承 f64 的舍入，极值及 2^53 附近的实际输出记录在日志；排序完全保留整数，range 的 I64 JSON 整数在转 f64 之前校验，2^53+1、i64 极值及小数形式端点明确拒绝，2^53 端点实际分桶核对。非有限 stats 结果报错。

所有新索引的配置快照随目录保存，FNV-1a 指纹只检测意外损坏，不是安全认证。只读重开测试比较包含配置在内的完整文件集合与字节。

## 公共 C/C++ 与旧二进制

同一份 `smoke/advanced_smoke.c` 分别用 C99 和 C++11 严格编译，真实链接宿主 DLL；每种语言以独立 create/reopen/readonly 进程完成验收：

```text
clang -std=c99 -Wall -Wextra -Werror -pedantic -Iinclude smoke/advanced_smoke.c -L.work/v3-host -ltantivy_ohos -o .work/v3-host/advanced_c.exe
clang++ -x c++ -std=c++11 -Wall -Wextra -Werror -pedantic -Iinclude smoke/advanced_smoke.c -L.work/v3-host -ltantivy_ohos -o .work/v3-host/advanced_cpp.exe
PASS Tantivy API v3 create
PASS Tantivy API v3 reopen
PASS Tantivy API v3 readonly
```

C 和 C++ 各自三个索引并存、关闭重开、只读、排序跨页、修改/删除持久化及未提交 ghost 不可见，均由程序断言。Python 从六份实际 JSON 日志独立核对：初始 stats count=7、sum=39，更新删除后 count=6、sum=36；初始分组高/低/中/空串=3/2/1/1，删除后=3/2/0/1；间隙范围 low/high=1/2。并测试错误/结果释放、排序值在句柄关闭后仍有效。

新最小 C++ 使用示例 `examples/advanced.cpp` 实际编译、链接和运行：三文档聚合 count=3、sum=25；字段倒序结果 b(price=20)、a(price=10)，score 不冒充 BM25。

实际复用修改前保存的 ABI 1 consumer 和 ABI 2 basic_c/basic_cpp 可执行文件，仅在独立执行目录放入新 DLL，不重编译这些 consumer：

```text
legacy ABI consumer read: PASS (runtime ABI 3)
id=doc-1 score=0.287682 fields=0
title=HarmonyOS 中文检索
category=Guide
year=2026
rating=4.5
[{"id":"a","text":"鸿蒙设备支持中文检索","score":0.28768212}]
a score=0.287682 fields=0
```

ABI 1 consumer 读取原库真实创建的旧索引副本，无新元数据；ABI 2 consumer 验证冻结的布局和新库的 v2 创建/读取行为。原 v2 测试程序源码的版本检查改为接受 >=2，以匹配累计 ABI 版本策略，其余行为断言保留。旧索引缺 fast field 的 v3 请求清楚报错，不自动改写 schema。

## OHOS 构建、链接及 Pura 状态

```powershell
.\build_ohos.ps1 -OpenHarmonySdk 'D:\Software\DevEco Studio\sdk\default\openharmony' -HostLlvm "$PWD/.tools/llvm-mingw-20261006-ucrt-x86_64"
```

release 构建通过。C99 和 C++11 的公开调用程序以 SDK clang/clang++、`--target=aarch64-linux-ohos --sysroot=<native/sysroot>` 编译链接通过。ELF 为 AArch64，SONAME 为 libtantivy_ohos.so，仅声明 DT_NEEDED libc.so。新接口与旧导出符号清单在日志中。

| 产物 | SHA-256 |
| --- | --- |
| 宿主 DLL | ffa941e0beb37a7a6273e6b9998ed67083b7cc7752b7aac6c0ce8cc2236cae00 |
| OHOS 原始库 | cd8a7769e49b2c107828175fe93fb7b06b6bb717a703043b235353a70ca9fa45 |
| 公开头文件 | bc89f655e4b23e17dbaf0bef51a6974bd6eeec44e677b5c2ea1416137f74ff19 |
| 本次安装源 signed HAP | dbd0d0def7f16cc0f7f339583c662b6ea04c9c5389a34f31dc3134f09c5ed968 |
| HAP 内经 strip 的库 | d868c041ddec68971009938cf9f624b74c9afabd849dfb266a3dca616f07b2aa |

新工程由 `prepare_api_smoke.ps1 -ApiVersion 3` 生成，仍使用用户授权的独立包 `com.torry2022.tantivyapiprobe`，复用其已有签名材料引用，没有修改签名材料。签名工程、材料和产物不会进入默认源码文件或审阅源码快照。HAP 解包核对包名与库哈希后，只在 `10.3.148.94:40013`（HBN-AL80）执行 install -r，返回 install bundle successfully。

启动使用全新 runId `api-v3-A-20261017-01`，没有覆盖此前 v2 报告。当前启动返回 **10106102：屏幕锁定，无法启动应用**，因此 **A 的真机执行待验**。没有使用此前 v2 的 Pura PASS 代替本批结果。没有更改开发者模式、安全配置或操作其他设备/实验包。尝试从独立调试沙箱取回 installed entry.hap 时路径不可访问，因此表中 HAP 哈希明确为实际安装命令使用的本地源文件，不冒充设备反向导出校验。

解锁后应以三次独立进程执行 create / reopen / readonly，比较 readonly 前后三个索引（含词典配置）的全部文件。主线已获知锁屏状态；本地 A 实现、文档和固定哈希先行交审，不因锁屏擅自开始 B。

## 使用边界与交审状态

- 新 v3 词典初始化目前每句柄各有 Jieba 实例；尚未测多 reader 词典内存成本，不预先引入共享缓存。
- PROJECT_NONE 仍为“不复制额外返回字段”，内部仍解码 stored document block 获取 ID；不是诊断 fast ID 路径。
- 本批没有引用旧 Zvec/whitespace/显式 OR 诊断的时延或内存数字。候选 SDK 的真实中文正确性与资源测量属于 B，需独立冻结原文、查询和配置。
- 内存限制是官方聚合估算守卫，不是进程 RSS 硬上限；terms 候选截断、f64 stats 精度、I64 range 精确端点及不支持重叠/嵌套的边界均写入公开头文件和中英文 README。
- 完整源码/头文件/构建产物对应关系见输出中的 A 候选 manifest；A 固定后等待主线复核。还未完成 B，未提交、推送或发布。
