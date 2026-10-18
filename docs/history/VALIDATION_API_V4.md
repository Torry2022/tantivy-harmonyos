# ABI 4：fast ID、可选精确命中数和公共 SDK 中文验证

本页为 B0 固定候选的历史记录；当前候选的显式 OR 验证、Count 评分一致性与非有限分数修复见 [补充记录](VALIDATION_API_V4_SUPPLEMENT.md)。下文同次收集描述对应 B0，后续候选已改为同快照独立 Count 扫描。

2026-10-18。新增功能不改变 ABI1/2/3 的函数签名、C 结构或旧 getter 含义；累计 ABI 版本为 4，v3 JSON version 仍为 3，v2 options 仍为 2。候选未提交、推送或发布。A 固定产物独立保留，本记录不能代替其尚未完成的真机执行。

## 实现与有针对性的证据

新 create_v3 的 fast_id 缺省在创建入口转成显式 true 并保存；Config 读取缺省仍是 None/false。id 原字段增加原样字符串 fast column，不添加调用方字段。A/旧元数据不含该键时仍按原 schema 校验；v2 创建不受影响。`fast_id:false` 可显式创建无 fast ID 的 v3 索引。

`src/api.rs` 的 materialize 在有效投影为空、且配置有 fast ID 时，通过各段 `fast_fields().str("id")`、`term_ords`、`ord_to_str` 获取 ID，并直接构造拥有字符串的结果；该分支在 `searcher.doc` 前 continue，不解码 stored document block。零命中不打开 fast ID 列。需要 stored 字段或旧索引缺 fast ID 时才走原 docstore 路径。

新增测试在唯一的 materialize docstore 读取处计数：31 篇大正文，ID-only 为 0 次，正文投影为 31 次；逐命中比较 ID、f32 score 位及全文，关闭句柄后结果仍有效。v2 空投影在新索引同样为 0 次；关闭重开后继续成立。这是分支与实际读取调用证据，不只检查 field_count=0。

search_v3 可选 track_total 默认 false。false 路径只执行原 TopDocs；true 路径用 `(TopDocs, Count)` 在同一 Searcher 和同次 search 收集页与总数。新增 result_total_v4 返回 total/exact；旧 result_count_v2 仍只给页长度。search_v2 的总数状态始终未统计，不隐式增加 Count 成本。

测试覆盖新索引和缺键的 A 格式：27 篇，排序升降序按页核对（含同值/缺失、超末页）；总数始终 27，而页长最多 5。writer 提交后旧 reader 的页和总数仍为旧快照，reload 后共同变为 26；零命中为 exact=true,total=0。未请求是 exact=false,total=0。错误 JSON、limit/offset 边界、空输出、结果释放与句柄关闭后计数均有断言。

## 实际通过的本地检查

- `cargo fmt -- --check`、`cargo test --locked --release -j 2 -- --nocapture`：16 项通过，保留 A 及 ABI2 的全部有效测试。日志中的既有 test panic 用来验证 C 边界捕获，该项通过。
- 公共 C99/C++11 `smoke/paging_smoke.c` 严格编译 `-Wall -Wextra -Werror -pedantic`；每种语言独立 create/reopen/readonly 三进程。复用 A 的全部词典、排序、聚合、持久化和未提交 ghost 断言，再检查 page=3,total=8、超末页 page=0,total=8、计数可选和 fast/body 一致。
- 复用修改前 ABI1、ABI2 C/C++ consumer 可执行文件，仅换新 DLL：通过。ABI1 读取原库创建的旧索引副本。原 A C consumer 二进制读取真实 A 索引：通过。
- ctypes 公共 ABI 再打开真实 A、旧格式和新索引，检查配置缺省、空投影、全部匹配数、reload、关闭后结果；完整索引文件集合、字节哈希和 mtime 均未改变。Rust 只读回归也保留。
- OHOS release 构建通过；AArch64 公共 C99/C++ 和基准调用程序链接通过。独立 B 验证 HAP、候选基准 HAP、ABI2 对照基准 HAP 均自动签名构建成功。基准长任务使用 taskpool，工程不进入源码快照。
- `examples/advanced.cpp` 实际编译运行：page=2,total=3,exact=1；字段排序 b=20,a=10，聚合 sum=25。

构建产物与最终源码/头文件对应哈希见 B 候选 manifest；构建成功不是设备运行成功。依赖、Cargo.lock、Rust 版本及上游源码未改。

## 原始中文公共 SDK 结果

流程和指标公式见 [基准说明](benchmarks/README.md)。使用冻结 ABI2 对照 DLL（SHA256 `9ab2a02275af58564985b7316f0d1bb23cbcd44aae859abe826e328ac0c15cc8`），全新独立 20,000 文档索引，与候选默认词典/原 QueryParser 对照。输入只读复制，哈希为：

| 输入 | SHA-256 |
| --- | --- |
| documents.original.tsv | 6f603df0da960ed58303969f722c187fede609f93eb491a40eb791358bd4646a |
| queries.original.tsv | 978995c34424fc8b64ffcface5f5f8d0f2110e5aa5e228ed148f0d50fca05e5f |
| qrels.tsv | 885d5ce672f921a459878102bbcb0f51f6f46d85d8cc73bf509ca87f73779fbf |

500 查询逐项排名、ID、分数位及返回正文一致，非同分差异 0，正文差异 0，本轮没有需要解释的同分差异。两边相同的查询 1592 和 4055 因原括号语法报错，没有改写；错误计入分母。实际返回正文在消费程序中逐字节核对原文。

| 指标（500 查询均值） | ABI2 对照 | 候选默认 |
| --- | --- | --- |
| nDCG@10 | 0.0301723113 | 0.0301723113 |
| MRR@10 | 0.068 | 0.068 |
| Recall@10 | 0.0204579188 | 0.0204579188 |
| Recall@100 | 0.0204579188 | 0.0204579188 |

这些数值对应原查询的默认 AND/短语解析语义，多数查询零命中；不是 OR 检索，不是完整 MTEB 任务，也不是经过检索调优后的能力上限。不能拿此前诊断的指标或耗时直接比较。

## 时间、内存及设备待验

宿主基准串行运行 baseline id/body、候选 fast id/body/countid/countbody、stored id、custom id，每场景 3 个独立进程，第二轮逆序，各进程先全量预热再重复 3 次。P50/P95、返回命中/字节、程序与 DLL 哈希、每个索引文件和体积记录在 HOST-REPORT.json。库调用计时不含请求构造和结果格式化；各返回需求与 Count 状态分开报告。宿主数字不用于宣称设备加速或 fast ID 总是更快，索引合并造成的实际体积差异也不冒充纯 fast 列成本。

Windows 无 PSS，memory 模式输出 -1，保持缺失。OHOS 基准已实现打开前、单/多 reader 打开/预热/关闭后的 Pss 采样；**Pura 上的同设备时间、PSS 增量和词典实例成本仍待实测**。因此未引入全局共享词典或长期缓存，未宣称内存优化通过。

Pura `10.3.148.94:40013` 重试已安装 A 包仍返回 10106102 锁屏。没有安装 B 或基准包，也没有操作 Mate/其他实验包。必须先以 A 固定包完成 create/reopen/readonly 和完整文件核对，再安装 B 做独立真机回归及基准。当前结论只到本地候选与基准准备，不是全目标验收完成；最终独立复核、A/B 真机验收和资源结果继续保留待验。
