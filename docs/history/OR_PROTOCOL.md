# 显式 OR 补充负载

这是公共 SDK 的独立负载补充。原始 500 查询的默认 QueryParser/AND 结果及两个语法错误完整保留，定位为默认语义兼容回归；其每轮合计 65 个返回结果不足以单独验收正文/ID 物化和查询后的 reader 内存。SDK 默认查询语义不变，不新增查询模式。

## 冻结与转义

`or_queries.rs` 是独立工具，链接当前 Cargo.lock 已构建的 jieba-rs 0.8.1 和锁定的 Tantivy query-grammar，不改依赖或 SDK。对批准的 `queries.original.tsv` 做 Jieba Default/HMM 分词，按 SDK 相同的 Rust `is_alphanumeric` 规则剔除纯标点、转小写，保留首次出现次序去重。每个词项用双引号包成字面量，依次转义反斜线和双引号，再以显式 ` OR ` 连接；不把查询文本拼成字段、范围、boost 等操作符。六组元字符/保留字测试和每条查询的 AST 字面值往返验证通过。

```powershell
# 在已配置的本地 GNU Rust 环境；不重新构建 SDK 或升级依赖
$jiebaLib = (Get-ChildItem target/release/deps/libjieba_rs-*.rlib).FullName
$grammarLib = (Get-ChildItem target/release/deps/libtantivy_query_grammar-*.rlib).FullName
rustc --edition=2021 -O benchmarks/or_queries.rs -L dependency=target/release/deps `
  --extern "jieba_rs=$jiebaLib" --extern "tantivy_query_grammar=$grammarLib" `
  -C "linker=$env:CARGO_TARGET_X86_64_PC_WINDOWS_GNU_LINKER" -o .work/or_queries.exe
python -B benchmarks/prepare_or.py .work/b-inputs <同源data目录> .work/or_queries.exe <新输出目录>
```

生成器独立重分词全部 500 条原查询，其 tokens TSV 与来源项目已经冻结的 queries.tokens.tsv **逐字节一致**；核对原查询、tokens 和原来源 manifest 哈希，不能只信文件名。输出清单继承原 mteb/T2Retrieval 固定 revision、Apache-2.0 许可声明和抽样/预处理说明，并记录原输入、派生文件、工具源码/二进制及 Cargo.lock 哈希。派生查询与 tokens 留在忽略目录，不提交数据。

上下文 HMM 词项再次单独送给原生 analyzer，未必仍是单 token。当前 2,794 个去重后词项中有 8 次重分析变化，例如原查询分出“图元”，独立字面量被分成“图 / 元”。这些词项全部保留，变化写入 native-literal-reanalysis.tsv；引号内内容由现有 QueryParser 按原生分词及短语语义解释。不能把它说成绕过 analyzer 的精确 TermQuery，也不能与旧 whitespace 诊断等同。

**正文仍是冻结的未预分词原文**，通过 ABI2 对照和候选各自公开 SDK 的原生 Jieba 建索引。没有改成 whitespace，没有使用私有检索入口，没有借用旧诊断的质量或资源数字。

## 排名、计数和覆盖检查

```text
sdk_bench query INDEX queries.explicit-or.tsv body 1 documents.original.tsv
sdk_bench query INDEX queries.explicit-or.tsv countbody 1 documents.original.tsv
sdk_bench query INDEX queries.explicit-or.tsv id 1
sdk_bench query INDEX queries.explicit-or.tsv countid 1
```

`analyze_or.py INPUTS RESULTS OUTPUT` 分开报告本负载的质量指标。定义沿用原始 qrels：nDCG@10、MRR@10、Recall@10/@100，对全部 500 查询求均值；未标注视为不相关。这些值反映显式 OR 调用语义，不能当作默认 AND 的质量，也不能拿旧诊断值当作本次结果。

正文模式实际逐字节核对冻结原文。排名分析器检查完整有序分数位序列和共同 ID 的分数；只允许同分组内交换，以及完整 Top100 的同一末尾分数组成员替换，其他分数组的成员和顺序都须一致。read() 检查每查询 ID 唯一、名次连续、页长、分数有限且降序以及查询记录不重复。所有非同分变化、正文错误或重复 ID 都拒绝。

`test_analyze.py` 从 B0 固定 ZIP 加载旧检查器，用高低分颠倒复现旧假通过，再证明新检查器报警。另测试错分/错正文、重复 ID、非法名次、组内同分交换及合法/非法边界替换。原 AND 输出只重算分析，不重跑旧计时矩阵。

报告每查询零命中率、返回数、正文字节分布（均值、P50/P90/P95/P99、极值），及每轮总量。Count 开关和 ID/body 投影分别比较全部排名/分数位；Count 的 total 必须不小于页长度，并在 ID/body 两种返回需求下逐查询相同。未请求仍明确标为未统计。

## 串行进程和设备准备

`run_or_host.py WORK WORKLOAD CANDIDATE_EXE BASELINE_EXE OUTPUT` 串行运行 8 个场景各 3 个独立进程；第二轮逆序，每进程全量预热后计时 3 轮。场景为 baseline id/body，候选 fast id/body/countid/countbody、stored id、custom id。保持原固定索引，核对所有文件名、字节和 mtime 前后不变。报告每进程 API P50/P95、PID、调用程序/DLL/输出哈希、返回需求、Count 状态和实际覆盖分布。测量期间不同时构建 SDK/HAP 或运行其他本任务基准。

计时只覆盖公开 C 函数调用，排除调用方构造请求、getter、整理输出和显式释放；库内 QueryParser、收集及物化属于 API 时间，不冒充纯引擎时间。显式 Count 现在在相同 Searcher 快照上另做扫描，其成本只由 count 场景承担。id/body 返回字节不同，不能把端到端输出整理成本算作单纯引擎差异。

新的基准工程仍由 prepare_app.ps1 生成，包名仍仅为自己的 `com.torry2022.tantivyapiprobe`，长任务使用 taskpool。新增 phase：

```text
orquery_fast_id_1 / orquery_fast_body_1 / orquery_fast_countid_1 / orquery_fast_countbody_1
ormemory_fast_1_id_1 / ormemory_fast_3_body_1
ormemory_custom_1_id_1 / ormemory_custom_3_body_1
```

OR phase 读取独立的 queries.explicit-or.tsv；原 query phase 继续读取 queries.original.tsv。memory 的数值分别是 reader 数和独立进程编号，模式可选 id/body/countid/countbody；default/custom 的 1/3 reader 至少分别测 id/body，每组重复 3 个独立进程。打开前、逐 reader 打开/预热后、全部关闭后采 Pss KiB，并记录实际预热返回数、字节和错误数，避免以零命中预热代替正文内存覆盖。Windows 返回 -1 表示无 PSS，不生成替代数值。

运行清单见补充快照中的 PURA-RUN-PLAN.json。仍需 **先以已安装固定 A 包完成 create/reopen/readonly 与文件核对，再安装 B1 功能验证包，最后比较基准包**。所有包/源码另存准确名称，不覆盖 A、B0 或带 v2 名称的旧交付；锁屏只暂停设备运行，设备时间/PSS和词典共享决策继续待验。
