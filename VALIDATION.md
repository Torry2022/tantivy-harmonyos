# 验证记录

本文件是原始 ABI 1 产物的历史记录。新增公共接口的实际检查与限制见
[ABI 2 验证记录](VALIDATION_API_V2.md)，下述真机结论不适用于新构建。

2026-10-17，验证以下组合：

| 项目 | 配置 |
| --- | --- |
| Tantivy | 0.26.2，crates.io 发布包 |
| Jieba-rs | 0.8.1，默认词典、Default 模式、HMM，加 lowercase |
| Rust | 1.90.0，Windows x86_64-pc-windows-gnu host |
| 宿主链接工具 | llvm-mingw 20261006 的 dlltool 与 Rust 自带 GNU 链接驱动 |
| 目标 | aarch64-unknown-linux-ohos |
| SDK | DevEco Studio 内置 OpenHarmony Native SDK |
| 真机 | Mate 90 Pro Max / CMM-AL10 |
| 设备系统参数 | OpenHarmony-7.0.0.109 |
| 测试包 | com.torry2022.retrievalprobe，独立调试签名 |

## 构建与打包

首次交叉构建通过；修正导出库的 SONAME 后，最终构建与重复构建均通过，重复构建的 SHA-256 一致。未修改 Tantivy 上游源码。

```text
libtantivy_ohos.so
size: 12679928 bytes
SHA-256: 465e2765fbf721395ce0e32667764476fc90a61db67c9b306fb57a9390c97938
SONAME: libtantivy_ohos.so
DT_NEEDED: libc.so
```

以上体积和依赖仅描述本次配置，不代表其他版本或构建选项。Windows 宿主工具未链接进目标库。

仓库内的测试工程模板已完成完整 HAP 构建；其本地副本复用同一测试包的签名 Profile，在真机安装并运行。私钥、证书、Profile、本地签名材料、本地副本、构建日志和二进制均不提交。

Hvigor 的 Native strip 阶段会改变包内库的哈希。本次已解包确认实际安装 HAP 包含该库，包内版本 SHA-256 为：

```text
cad21bd73d453815c6f5a8030fad8660a5d2bfc1cf7560ea8079bc4c91206919
```

复核发现并修正了两项集成问题：

- 无 SONAME 时，CMake 可能把开发电脑绝对路径写入调用方的 DT_NEEDED，导致真机无法加载。
- 用 IMPORTED_NO_SONAME 绕过路径问题会使本次 Hvigor 的运行依赖收集遗漏导入库。最终由 Rust 输出正确 SONAME，CMake 使用正常的 SHARED IMPORTED 配置，并检查 HAP 实际包含该库。

## 宿主测试

`cargo test --locked --release -j 2`：3 项通过，0 失败。

- 空 handle、非法 UTF-8 的错误返回，以及错误字符串释放。
- 中文、emoji 和混合文本的 tokenizer 字节偏移与位置。
- 提交前后可见性、中文 / 混合 / 布尔 / 短语查询、单 writer 锁、拒绝覆盖已有索引、关闭重开、删除和更新、未提交写入不被重开后的 reader 看见。

## 最终产物真机验证

### 运行测试

构建库后运行 `./prepare_smoke.ps1`，在 DevEco 打开 `smoke/harmony`，为测试包配置调试签名。可以复用已有证书和密钥，新包名只需关联相应的 Profile。

默认包名为 `com.torry2022.retrievalprobe`，三个阶段需分别运行于独立进程：

```sh
hdc -t <device> shell aa start -a EntryAbility -b com.torry2022.retrievalprobe --ps phase create
hdc -t <device> shell aa force-stop com.torry2022.retrievalprobe
hdc -t <device> shell aa start -a EntryAbility -b com.torry2022.retrievalprobe --ps phase reopen
hdc -t <device> shell aa force-stop com.torry2022.retrievalprobe
hdc -t <device> shell aa start -a EntryAbility -b com.torry2022.retrievalprobe --ps phase verify
```

默认目录为 `filesDir/trial-01`。再次测试时，为三个阶段都添加相同的新 `--ps runId trial-02`，保留旧数据并改用新目录。测试程序不会删除已有索引。各阶段报告保存在相应目录的 `report-tantivy-*.txt`、`details-tantivy-*.log` 和 `errors-tantivy-*.log` 中。

### 验证结果

使用准备提交的独立测试工程和上述最终库，在应用 filesDir/final-20261017 下创建新的测试索引。没有使用聊天 App 的目录或已有实验 collection。

三个阶段分别运行于独立进程；阶段之间仅停止测试包自身。

```text
Tantivy create: PASS
Tantivy reopen: PASS
Tantivy verify: PASS
```

覆盖：

1. 创建索引、写入 3 条中文文档，提交前不可查询，提交后返回正确 ID、原文和 BM25 分数。
2. 中文关键词、Qwen3、16GB、布尔 AND、短语查询、无命中和语法错误返回。
3. 进程退出后重开，原结果保留；删除 b、更新 a 并提交后，旧关键词不再命中。
4. 再次退出并重开，删除和更新后的结果保持一致。

## 尚未覆盖

大规模语料、召回率或相关性评测、冷 / 热延迟和内存基准、LLM 已加载时的资源竞争、断电和磁盘写满、并发调用压力、不同系统版本与 ABI、领域词典以及版本间索引迁移。

本记录证明已验证组合中的基础适配和调用链，不据此宣称全功能兼容、生产稳定性或性能优于其他引擎。
