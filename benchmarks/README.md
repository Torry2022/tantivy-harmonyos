# 公共 SDK 原始中文基准

原始 AND 负载用于默认语义兼容回归。实际返回内容充足的补充性能负载、严格排名检查与当前 Count 扫描说明见 [显式 OR 协议](OR_PROTOCOL.md)。

本目录的调用程序只使用公开 C ABI，不修改 Tantivy tokenizer、QueryParser 或依赖。支持 Windows 宿主诊断和 OHOS 应用沙箱运行。设备结果须由实际运行产生；构建成功不代表 Pura 验证通过。

## 输入和查询语义

```powershell
python -B benchmarks/freeze_inputs.py <批准的数据目录> .work/b-inputs
```

只复制 `documents.original.tsv`、`queries.original.tsv`、`qrels.tsv`，要求哈希与来源 `manifest.json` 匹配；目标已存在则拒绝覆盖。语料不进入源码或审阅包。当前输入为 mteb/T2Retrieval revision `921dd3af6e78d1ae7ee0368aa8d7eaee02c8f08e` 的固定 20,000 文档、500 查询子集和 2,658 个原始正相关标注，来源清单声明 Apache-2.0。`INPUT-MANIFEST.json` 保留来源、许可声明、抽样/预处理说明、输入哈希及行数。这是子集评估，不是完整 MTEB 分数。

正文是来源项目已清理 HTML、合并空白后的原始文本，不再分词、切块或改写。原查询字符串直接传给 SDK QueryParser，使用默认 Jieba Default/HMM、小写及隐式 AND；其中单个词项产生的多 token 按上游解析语义处理，不改为 OR。括号等解析错误保留并计入全部 500 查询的质量分母，不自动转义或丢弃。不得与旧 whitespace/显式 OR 诊断的分数、时延或内存直接比较。

`sdk_bench.cpp` 以同一顺序写入正文，Top100；每 5,000 文档显式提交，最终提交并关闭。不强制上游段合并时序，记录实际全部索引文件哈希和大小。对照使用冻结 ABI2 公共 SDK 原库和头文件；候选的 fast/stored 使用相同默认词典，custom 仅用于词典配置的资源观察，不用来宣称默认语义不变。

## 正确性与指标

使用 `query INDEX QUERIES body 1 DOCS` 做正文核对：程序将每个实际返回正文逐字节与冻结原文比对。TSV 保存查询 ID、排名、文档 ID、f32 分数位、正文长度及 FNV-1a 摘要（摘要只用于对照，完整字节断言已在调用程序内执行）。

```powershell
python -B benchmarks/analyze.py .work/b-inputs <对照结果.tsv> <候选结果.tsv> <输出.json>
```

分析保留实际排名，不对结果重排。共同文档分数位必须一致；完整分数组仅允许同分项交换次序，Top100 边界也只允许同一分数的候选差异，差异逐条写入报告。不同分数的差异不会作为“同分”放过。正文差异失败。

- nDCG@10：实际 DCG 除以理想 DCG；gain=`2^grade-1`，第 r 名折扣为 `log2(r+1)`。
- MRR@10：前十中首个正相关结果的名次倒数，无则 0。
- Recall@10 / @100：截断内正相关数量除以该查询的全部已标注正相关数量。
- 未标注文档视为不相关；解析错误、零命中查询贡献 0。对全部查询取均值，不只统计有命中的查询。这衡量固定 QueryParser 语义与标注的匹配程度，不是对中文检索任务调参后的质量上限。

## 时间与资源

同一 C++ 程序分别以 ABI2 宏和候选头文件编译。CLI：

```text
sdk_bench build INDEX DOCS fast|stored|custom
sdk_bench query INDEX QUERIES id|body|countid|countbody REPEATS [DOCS]
sdk_bench memory INDEX QUERIES READERS
```

对照 ABI2 不支持 Count。候选 id/body 默认不计数；countid/countbody 显式精确计数。每个新进程先完整预热 500 查询，再做 3 轮查询。库调用计时从实际 C 函数调用前到返回后：包含 SDK 内解析、收集及结果物化，排除调用方请求构造、getter 遍历、字符串整理、stdout 和显式结果释放。TSV 的最后一列另记到结果处理结束的耗时，包含调用方处理，不混称纯引擎时间。正文核对模式不用于资源比较；不在 timed 模式加载原文映射。

`run_host.py WORK [--output 新目录]` 串行运行每个场景的 3 个独立进程，第二轮倒转场景顺序，保留 PID、命令、程序/DLL/输出哈希、实际返回命中数和正文字节数、每进程 P50/P95，以及索引文件哈希/体积。输出目录不可覆盖。宿主数据只诊断本地行为；不能替代同一 Pura 上的测量。默认 AND 的多数零命中也会影响时间分布，不以这组结果宣称 fast ID 一定更快。

memory 不加载正文映射，通过 `/proc/self/smaps_rollup` 报告 Pss（KiB）：打开前、每个 reader 打开后、预热后、全部关闭后。分别以 1/3 reader，对 default/custom 运行 3 个新进程，报告相对打开前的增量及每 reader 增量。Windows 或读取不支持时值为 -1，表示缺失，不以 RSS/working set 冒充 PSS。关闭后 allocator 保留页不等于词典仍被全局缓存；v3 已按完整词典配置共享不可变 Jieba，池只持有弱引用，最后使用者释放后实例及其词典配置可回收，过期弱项在下次访问时清理。同设备、同索引的测量见[共享验证记录](../VALIDATION_SHARED_JIEBA.md)；既有 A/B1 数据继续对应各自冻结实现。

## OHOS 应用准备

`prepare_app.ps1` 只生成独立包 `com.torry2022.tantivyapiprobe` 的工程；传入相应库及头文件目录，ABI2 对照加 `-BaselineAbi2`。拒绝覆盖已有工程，不安装或更改设备。长基准在 taskpool 线程运行，避免阻塞 ArkUI 主线程；native 调用程序与宿主相同。

```powershell
./benchmarks/prepare_app.ps1 -Destination build/bench-v4-harmony `
  -Library out/aarch64-unknown-linux-ohos/libtantivy_ohos.so -HeaderDirectory include
```

为自己的包配置匹配 Pura 的调试 Profile，构建后记录 HAP 和包内实际库哈希。运行参数仍为 EntryAbility 的 phase/runId。每个引擎独立 runId，数据保存在该目录下；包替换安装需保留沙箱数据。`prepare_fast` 创建目录；将冻结的两份原文/查询 TSV 传入该独立调试沙箱并核对哈希，然后执行：

```text
build_fast / build_stored / build_custom
query_fast_id_1 / query_fast_body_1 / query_fast_countid_1 / query_fast_countbody_1
memory_fast_1_1 / memory_fast_3_1 / memory_custom_1_1 / memory_custom_3_1
```

query 的末尾 1/2/3 是独立进程编号，每进程内部仍重复 3 次；memory 的两个数字是 reader 数和进程编号。各 phase 之间仅 force-stop 自己的包，核对报告中的 PID 不同，并收取 details/errors/report 文件。基准前后核对索引全部文件；只读查询不应改名、改字节或 mtime。重跑使用新 runId，不覆盖结果。

设备运行有顺序要求：**先补完已安装 A 固定包的 create/reopen/readonly 及文件核对，再安装 B 或对照基准包**。A 的源码/库/HAP 固定副本不被本目录改写。若设备不可用，记录待验，不能用 B 结果替代 A 的待验项，也不能用宿主 PSS 缺失或历史实验数字填补设备结果。
