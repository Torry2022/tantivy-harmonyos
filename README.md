# tantivy-harmonyos

Tantivy 0.26.2 的非官方 HarmonyOS arm64 适配，提供可复现的 Windows PowerShell 构建脚本、精简 C API、中文分词和独立真机测试工程。

用于在鸿蒙 Native 模块中验证本地全文检索，不是 Tantivy 或华为的官方项目。第一版采用固定的文档 ID / 正文字段，未接入 Turbo AI Chat，也不要求同时引入向量数据库。

## 提供的能力

- Jieba 中文分词，保留 UTF-8 字节偏移，并统一小写；词典随库编入。
- BM25 排序、中文与中英混合查询、布尔和短语查询。
- 创建和打开索引、按 ID 写入或替换、删除、提交与持久化。
- C ABI 返回文档 ID、原文和分数，JSON 与错误字符串均有对应释放函数。
- 独立测试 HAP，验证 ArkTS → Node-API / C++ → Rust → Tantivy 调用链。

这不是 Tantivy 全量 API 的映射。句法、分词和字段定义见 [C 头文件](include/tantivy_ohos.h)；已有索引必须符合本适配器的 schema。

## 预编译库

从 [v0.26.2-ohos.1 Release](https://github.com/Torry2022/tantivy-harmonyos/releases/tag/v0.26.2-ohos.1) 下载 `tantivy-0.26.2-harmonyos-arm64-v8a.zip`，并使用同页的 `SHA256SUMS` 校验：

```powershell
Get-FileHash .\tantivy-0.26.2-harmonyos-arm64-v8a.zip -Algorithm SHA256
```

压缩包包含 arm64 动态库、C 头文件、源码和未签名测试工程、构建清单与第三方许可。库中已包含默认中文词典，无需另外下载词典文件；按下方 Native 调用说明集成即可。版本号跟随 Tantivy 上游，`ohos.1` 表示本项目的首次适配发布；C API 适配器版本为 0.1.0。

## 构建

已验证的构建组合为 Windows、Rust 1.90.0（GNU host）、DevEco OpenHarmony Native SDK 和 arm64 OHOS 目标。Tantivy 与 Jieba-rs 分别固定为 0.26.2、0.8.1；完整依赖固定在 Cargo.lock。

准备 Rust 1.90.0 和目标标准库：

```powershell
rustup toolchain install 1.90.0 --profile minimal
rustup target add aarch64-unknown-linux-ohos --toolchain 1.90.0
```

Windows GNU host 需要可用的 64 位 MinGW 链接工具。测试使用 [llvm-mingw 20261006 UCRT x86_64](https://github.com/mstorsjo/llvm-mingw/releases/tag/20261006)；仅有 32 位 MinGW 不够。Rust 标准库和宿主链接工具不是鸿蒙目标库，最终目标代码仍由 OHOS Clang / sysroot 链接。

```powershell
.\build_ohos.ps1 `
  -OpenHarmonySdk 'D:\path\to\DevEco Studio\sdk\default\openharmony' `
  -HostLlvm 'D:\path\to\llvm-mingw-20261006-ucrt-x86_64'
```

脚本使用 `cargo rustc --locked`，默认最多两个构建任务，仅对当前进程配置工具链，退出后恢复环境变量。不会修改系统 PATH，也不会自动下载或改动上游源码。输出：

```text
out/aarch64-unknown-linux-ohos/
  libtantivy_ohos.so
  tantivy_ohos.h
```

链接时显式设置 `libtantivy_ohos.so` 的 SONAME，避免 CMake 把开发电脑的路径写入调用方。当前没有对 Tantivy 上游源码打补丁。构建中仍有传递压缩依赖，脚本会给其设置 OHOS C/C++ 编译器和归档工具。

## Native 调用

将头文件加入 include 路径，将 `.so` 放进模块的 `entry/src/main/libs/arm64-v8a/`，在 CMake 中导入并链接：

```cmake
add_library(tantivy_ohos SHARED IMPORTED)
set_target_properties(tantivy_ohos PROPERTIES
    IMPORTED_LOCATION "${CMAKE_CURRENT_LIST_DIR}/../libs/arm64-v8a/libtantivy_ohos.so")
target_link_libraries(entry PRIVATE tantivy_ohos)
```

最终 HAP 必须包含该库。索引路径应位于应用可写沙箱中，从 ArkTS 获取 filesDir 后传入；不要硬编码其他应用的沙箱路径。

调用顺序：

```text
open(create=true) → upsert → commit → search → close
open(create=false) → search → delete / upsert → commit → search → close
```

- `create=true` 仅创建新目录，不覆盖已有目录；父目录需已存在。
- `commit` 同时刷新本句柄的 reader；未提交的修改不参与查询，close 不隐式提交。
- 同一索引仅允许一个 writer。同一 handle 的调用必须由调用方串行化，包括 close。
- 查询使用 Tantivy QueryParser，默认词项之间为 AND；语法错误通过错误返回值报告。
- 使用 `tantivy_ohos_free_string` 释放返回字符串，不要使用 `free`、`delete` 或 Rust 之外的分配器。
- 返回的分数只在相关查询和配置下解释，不应直接与向量相似度分数相加。

## 测试

宿主功能测试：

```powershell
cargo test --locked --release -j 2
```

GNU host 需在当前终端配置好上述 64 位工具链。真机测试工程准备方式：

```powershell
.\prepare_smoke.ps1
```

然后在 DevEco 打开 `smoke/harmony`，配置测试包自身的调试 Profile 并安装。可以复用已有调试证书与密钥；新包名的 Profile 不等于新建调试证书。不要借用业务 App 的包名或替换正在实验的安装包。

测试工程默认包名为 `com.torry2022.retrievalprobe`。三个阶段使用同一测试索引，阶段之间必须退出进程：

```sh
hdc -t <device> shell aa start -a EntryAbility -b com.torry2022.retrievalprobe --ps phase create
hdc -t <device> shell aa force-stop com.torry2022.retrievalprobe
hdc -t <device> shell aa start -a EntryAbility -b com.torry2022.retrievalprobe --ps phase reopen
hdc -t <device> shell aa force-stop com.torry2022.retrievalprobe
hdc -t <device> shell aa start -a EntryAbility -b com.torry2022.retrievalprobe --ps phase verify
```

测试默认使用 filesDir/trial-01。重测时，在三个阶段的启动命令中使用相同的新 `--ps runId trial-02`，即可保留旧数据、改用新测试目录。样例不会自行删除已有索引。结果保存在相应 trial 目录下的 `report-tantivy-*.txt`、`details-tantivy-*.log` 和 `errors-tantivy-*.log`。

测试源码在 [smoke/tantivy_smoke.cpp](smoke/tantivy_smoke.cpp)，真机记录与覆盖边界见 [VALIDATION.md](VALIDATION.md)。

## 边界

- 当前仅验证 arm64；未宣称 x86_64 模拟器或全部 HarmonyOS 版本兼容。
- 不包含向量召回、混合检索融合、Embedding 或 UI 产品功能。
- 采用 Jieba 默认模式加 HMM，不包含领域用户词典、搜索模式同义词或相关性调优。
- 不保证与其他 Tantivy 版本、schema 或分词配置的索引兼容。
- 小样本正确性测试不等于性能、检索质量、断电恢复或并发压力验收。
- 已测试的动态库只依赖系统 libc；它仍包含 Rust 及若干依赖的静态代码，分发时需保留相关许可。预编译包同时附带依赖许可与来源材料。

## 许可证

适配代码和脚本采用 MIT。Tantivy、Jieba-rs 及其他依赖保留各自许可，见 LICENSE、NOTICE、third_party/ 与 Cargo.lock。预编译包的 `licenses/` 包含依赖、Rust 标准库和词典许可材料，`BUILD-MANIFEST.json` 记录来源提交、依赖版本和词典哈希。再分发时请保留这些材料。

维护者打包时，在与构建相同的 Rust 环境中运行 `python package_release.py`（需要 Python 3）。脚本从锁定的 OHOS 依赖图收集许可，加入已验证的动态库、Git 已跟踪源码及文件校验和，输出到 `build/release/`；不会包含本地签名和构建目录。
