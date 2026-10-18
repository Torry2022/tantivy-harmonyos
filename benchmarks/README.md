# 中文检索基准工具

`sdk_bench.cpp` 使用当前公开接口，可测同一索引的 ID/正文投影、精确计数和多 reader 内存。它与示例一起由 `examples/CMakeLists.txt` 编译，只能使用配套的库与头文件。

```text
sdk_bench build INDEX DOCS fast|stored|custom
sdk_bench query INDEX QUERIES id|body|countid|countbody REPEATS [DOCS]
sdk_bench memory INDEX QUERIES READERS
```

DOCS 和 QUERIES 为两列 TSV：ID、文本。输入不能含额外制表符；查询直接使用 QueryParser 默认 AND 语义，不自动改写。`fast` 使用默认 ID fast column，`stored` 关闭它，`custom` 添加示例词典，仅用于观察词典资源开销。写入每 5000 条显式提交，最终提交后关闭，不强制合并段。

`query` 先预热，再重复计时。API 耗时从进入 C 查询调用到返回，不含请求构造、结果遍历、输出和显式释放。用 `body REPEATS DOCS` 可逐字节核对返回正文。TSV 保存文档 ID、排名、分数位和正文摘要。`memory` 读取 `/proc/self/smaps_rollup` 的 PSS（KiB），包括打开前、逐个 reader 打开后及关闭后；不支持时返回 -1，不以 RSS 冒充 PSS。索引体积为整个目录的实际文件总字节数。

`freeze_inputs.py` 复制并校验来源清单中的 documents.original.tsv、queries.original.tsv 和 qrels.tsv，目标存在时拒绝覆盖。语料不纳入仓库。`analyze.py` 比较两份结果的排序、分数和正文，并计算 nDCG@10、MRR@10、Recall@10/100；解析失败和零命中保留在全部查询的分母中。对比时须统一语料、分词、查询语义、返回字段、计数需求及索引配置，保存输入与产物哈希。

HarmonyOS 独立测试工程：

```powershell
.\benchmarks/prepare_app.ps1 -Destination build/benchmark-harmony `
  -Library out/aarch64-unknown-linux-ohos/libtantivy_ohos.so -HeaderDirectory include
```

在 DevEco 配置该测试包自己的 Profile 后构建。脚本不覆盖已有工程，也不安装或操作设备。EntryAbility 通过 phase/runId 运行各阶段；每次使用新 runId，不覆盖旧结果，仅停止自己的测试包。宿主结果不能当作真机性能。

旧接口的实验记录位于 [docs/history](../docs/history/)，仅对应其中固定的源码和产物，不作为当前版本验证结论。
