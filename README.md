# tantivy-harmonyos

[简体中文](README.md) | [English](README_EN.md)

为 HarmonyOS arm64 应用提供 [Tantivy](https://github.com/quickwit-oss/tantivy) 全文检索，通过 C API 在 Native 模块中调用。支持 Jieba 中文分词、BM25 排序、布尔与短语查询，以及索引的写入、更新、删除和持久化。

本项目为非官方适配，基于 Tantivy 0.26.2 和 Jieba-rs 0.8.1，未修改 Tantivy 上游源码。当前接口使用固定的文档 ID 和正文字段，适合以文档为单位检索；不包含 Tantivy 的全部 API。

## 下载

在 [Releases](https://github.com/Torry2022/tantivy-harmonyos/releases) 下载 `tantivy-0.26.2-harmonyos-arm64-v8a.zip`。包内包含动态库、C 头文件、源码、测试工程及依赖许可，中文词典已编入库中。

下载后可与同页的 `SHA256SUMS` 核对：

```powershell
Get-FileHash .\tantivy-0.26.2-harmonyos-arm64-v8a.zip -Algorithm SHA256
```

发布版本中的 `0.26.2` 对应 Tantivy 版本，`ohos.1` 是本项目的适配版本。

## 使用

将 `libtantivy_ohos.so` 放入模块的 `entry/src/main/libs/arm64-v8a/`，将 `tantivy_ohos.h` 加入头文件搜索路径。在 CMake 中链接动态库（以下假设调用方目标名为 `entry`）：

```cmake
add_library(tantivy_ohos SHARED IMPORTED)
set_target_properties(tantivy_ohos PROPERTIES
    IMPORTED_LOCATION "${CMAKE_CURRENT_LIST_DIR}/../libs/arm64-v8a/libtantivy_ohos.so")
target_link_libraries(entry PRIVATE tantivy_ohos)
```

索引保存在应用的可写沙箱中，可从 ArkTS 获取 `filesDir` 后传给 Native 模块。基本调用顺序：

```text
open(create=true) → upsert → commit → search → close
open(create=false) → search → delete / upsert → commit → close
```

接口定义见 [tantivy_ohos.h](include/tantivy_ohos.h)，完整调用示例见 [tantivy_smoke.cpp](smoke/tantivy_smoke.cpp)。使用时需注意：

- 创建索引需要提供尚不存在的目录，其父目录应已存在。打开已有索引时，字段和分词配置必须与本适配器一致。
- 写入和删除在 `commit` 后对查询可见，`commit` 会刷新当前句柄的 reader；`close` 不会自动提交。
- 一个索引只能有一个 writer，同一句柄的操作（包括关闭）须串行执行。
- 查询结果为包含 `id`、`text` 和 `score` 的 JSON 数组。结果和错误字符串均由 `tantivy_ohos_free_string` 释放。

中文分词使用 Jieba 默认词典、默认模式和 HMM，并统一小写。查询采用 Tantivy QueryParser 语法，空格分隔的词默认按 AND 处理，也支持显式布尔运算与双引号短语。当前未提供自定义字段或领域词典接口。

## 从源码构建

需要 Windows、Rust 1.90.0 GNU 工具链、64 位 MinGW 工具和 DevEco OpenHarmony Native SDK。已使用的 MinGW 工具为 [llvm-mingw 20261006 UCRT x86_64](https://github.com/mstorsjo/llvm-mingw/releases/tag/20261006)。

在仓库目录的 PowerShell 中准备 Rust：

```powershell
rustup toolchain install 1.90.0-x86_64-pc-windows-gnu --profile minimal
rustup target add aarch64-unknown-linux-ohos --toolchain 1.90.0-x86_64-pc-windows-gnu
$env:RUSTUP_TOOLCHAIN = '1.90.0-x86_64-pc-windows-gnu'
```

指定本机 SDK 和 MinGW 路径后构建：

```powershell
.\build_ohos.ps1 `
  -OpenHarmonySdk 'D:\path\to\DevEco Studio\sdk\default\openharmony' `
  -HostLlvm 'D:\path\to\llvm-mingw-20261006-ucrt-x86_64'
```

脚本按 `Cargo.lock` 构建，默认使用两个并行任务。目标库由 OHOS Clang 和 sysroot 链接，输出到 `out/aarch64-unknown-linux-ohos/`：

```text
libtantivy_ohos.so
tantivy_ohos.h
```

运行宿主测试和准备真机测试工程：

```powershell
cargo test --locked --release -j 2
.\prepare_smoke.ps1
```

随后在 DevEco 中打开 `smoke/harmony`，配置调试签名并安装。测试步骤、真机记录及尚未覆盖的场景见 [VALIDATION.md](VALIDATION.md)。目前已验证 HarmonyOS arm64，不代表其他架构或所有系统版本均兼容。

## 许可证

适配代码与脚本采用 [MIT](LICENSE)。Tantivy、Jieba-rs 及其他依赖保留各自许可，见 [NOTICE](NOTICE) 和 [third_party/](third_party/)。预编译包中的 `licenses/` 包含依赖、Rust 标准库和词典的许可，`BUILD-MANIFEST.json` 记录构建来源；再分发时请一并保留。

维护者可在构建环境中运行 `python package_release.py` 生成发布包，输出至 `build/release/`。
