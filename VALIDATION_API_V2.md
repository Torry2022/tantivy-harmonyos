# 公共 C API（v2）验证

2026-10-17，基线 `45d04d397e64d3bfa2456ae6332bf4b396eb4cfe`。独立克隆位于 `D:\DevCode\HarmonyOS\tantivy-harmonyos-api`，分支 `codex/public-c-api`。没有修改或构建原始 checkout；没有提交、推送、打 tag 或发布。

## 环境与固定版本

- Rust `1.90.0 (1159e78c4 2025-09-14)`，host `x86_64-pc-windows-gnu`。
- Tantivy 0.26.2、jieba-rs 0.8.1；`Cargo.toml`、`Cargo.lock`、`rust-toolchain.toml` 没有改变，也没有上游源码补丁。
- 宿主工具为独立复制的 llvm-mingw 20261006 UCRT x86_64；clang 报告 23.1.3。
- 目标 `aarch64-unknown-linux-ohos`；DevEco SDK 路径 `D:\Software\DevEco Studio\sdk\default\openharmony`。
- 所有构建和缓存位于独立克隆内。下述 `.work` 内容是本地验证材料，未加入产品源码。

本地环境设置（在独立克隆根目录执行）：

```powershell
$env:RUSTUP_HOME = (Resolve-Path .tools/rustup).Path
$env:CARGO_HOME = (Resolve-Path .tools/cargo).Path
$env:RUSTUP_TOOLCHAIN = '1.90.0-x86_64-pc-windows-gnu'
$env:PATH = "$env:CARGO_HOME/bin;$(Resolve-Path .tools/llvm-mingw-20261006-ucrt-x86_64/bin);$env:PATH"
$rustSysroot = (& rustc --print sysroot).Trim()
$env:CARGO_TARGET_X86_64_PC_WINDOWS_GNU_LINKER = "$rustSysroot/lib/rustlib/x86_64-pc-windows-gnu/bin/self-contained/x86_64-w64-mingw32-gcc.exe"
```

`.tools` 来自只读复制的已有工具链，不依赖原 checkout 的可变缓存。最初未规范化工具路径时，GNU 驱动未找到 ld；改用规范化的工具链绝对路径后通过。没有更换编译器版本或锁文件。

## Rust 宿主测试

原提交的 Cargo 文件和 src 文件通过 `git show HEAD:<path>` 写入 `.work/baseline`，在独立克隆中构建，原始 3 项测试全部通过：

```powershell
cargo test --manifest-path .work/baseline/Cargo.toml --target-dir target --locked --release -j 2
```

新源码：

```powershell
cargo fmt -- --check
cargo test --locked --release -j 2
cargo build --locked --release --lib -j 2
```

结果：**10 passed, 0 failed**。保留原有三项测试的断言；原测试访问 writer 的一处实现细节改为调用等价 delete，fixture 路径助手向兄弟测试模块开放。

覆盖：

1. 原有中文、emoji 字节偏移、Qwen3 / 16GB、布尔、隐式 AND、短语、查询语法错误。
2. 原 id/text 索引（没有适配器元数据）读写/只读重开和 JSON 查询；旧接口拒绝自定义配置。
3. 自定义 TEXT、KEYWORD、I64、F64，保存和索引独立选项，默认查询字段与单次查询覆盖，字段限定数值范围、精确大小写。
4. DEFAULT、NONE、FIELDS 三种投影；数值 view、空字符串、缺失字段略过、不保存字段拒绝返回；结果在提交、reload、关闭所有句柄后仍可读。
5. 提交前不可见、commit 刷新本句柄、其他 reader 必须 reload、多个 reader 与单 writer 共存、第二 writer 冲突。
6. 同一 ID 连续替换、整文档替换后遗漏字段消失、删除和更新重开持久化、关闭不自动提交。
7. 只读句柄没有 writer，写入/删除/commit 明确报错；打开、查询、reload、关闭前后比较文件集合、全部字节和修改时间，完全相同；缺失元数据锁时失败且不创建文件。
8. 只读元数据锁与 Tantivy 默认锁双向阻塞，释放后成功获取，验证其与默认文件回收锁互操作。
9. 元数据损坏、不支持的 tokenizer、配置与真实 schema 不匹配、非原始 schema 缺元数据时拒绝重开。
10. NULL、非法 UTF-8、错误版本/大小/模式/标志、非法/重复字段、错误类型、NaN / Infinity、错误数组组合、投影、limit 和结果越界；验证错误及结果释放、输出清零、校验失败不排入删除。边界 helper 捕获可展开 panic。

这不是失效指针、调用方越界内存或内存耗尽的防护测试。未用删测试、放宽断言或吞掉检查失败来制造通过。

## 真实 C/C++ 调用与旧二进制兼容

用 llvm-mingw 的 clang / clang++，所有例子按 `-Wall -Wextra -Werror -pedantic` 编译链接；C 用 C99，C++ 用 C++11。链接本次宿主 DLL：

```powershell
clang -std=c99 -Wall -Wextra -Werror -pedantic -Iinclude examples/basic.c -L.work/host-v2-bin -ltantivy_ohos -o .work/host-v2-bin/basic_c.exe
clang++ -std=c++11 -Wall -Wextra -Werror -pedantic -Iinclude examples/basic.cpp -L.work/host-v2-bin -ltantivy_ohos -o .work/host-v2-bin/basic_cpp.exe
clang -std=c99 -Wall -Wextra -Werror -pedantic -Iinclude smoke/tantivy_api_smoke.c -L.work/host-v2-bin -ltantivy_ohos -o .work/host-v2-bin/api_smoke.exe
```

实际调用结果：

```text
id=doc-1 score=0.287682 fields=0
title=HarmonyOS 中文检索
category=Guide
year=2026
rating=4.5
[{"id":"a","text":"鸿蒙设备支持中文检索","score":0.28768212}]
a score=0.287682 fields=0
PASS Tantivy API v2 create
PASS Tantivy API v2 reopen
PASS Tantivy API v2 verify
PASS Tantivy API v2 readonly
```

四个阶段分别运行在独立宿主进程中，使用新目录 `.work/native-api-index`。

另外从原提交构建原 DLL，以原头文件编译 [legacy_abi.c](tests/legacy_abi.c)，用该 DLL 创建真实旧索引（无 `tantivy-ohos.json`）。然后仅替换执行目录内的 DLL 为本次新 DLL，不重新编译 C 程序，也不迁移索引：

```text
legacy ABI consumer create: PASS (runtime ABI 1)
legacy ABI consumer read: PASS (runtime ABI 2)
```

签名、布局和旧查询行为兼容，但 `tantivy_ohos_abi_version()` 返回值升级为 2；旧调用方若严格要求 `== 1`，需更新版本接受条件。v2 冻结的 options/version/size 契约见公开头文件。

## OHOS arm64 构建及链接

```powershell
.\build_ohos.ps1 -OpenHarmonySdk 'D:\Software\DevEco Studio\sdk\default\openharmony' -HostLlvm "$PWD/.tools/llvm-mingw-20261006-ucrt-x86_64"
```

结果：release 构建通过。未改变原构建脚本。

```text
libtantivy_ohos.so
size: 12812328 bytes
SHA-256: 2e191851a12ae7706602cb12c80a7aa029a3b34428c466025ab0eedaacb0e373
Machine: AArch64
SONAME: libtantivy_ohos.so
DT_NEEDED: libc.so
```

使用 SDK clang/clang++、`--target=aarch64-linux-ohos --sysroot=<native/sysroot>`，相同严格警告选项，编译并链接 `smoke/tantivy_api_smoke.c`、`examples/basic.c`、`examples/basic.cpp` 成功。C++ 调用程序另依赖 `libc++_shared.so`，适配库自身仅声明依赖 `libc.so`。ELF 检查未发现调用方 DT_NEEDED 引入开发电脑绝对路径。

## Pura 与独立应用工程

用户指定 Pura 的连接为 `10.3.148.94:40013`，只对该目标执行了相关命令。型号 HBN-AL80，报告 `HBN-AL80 7.0.0.109(SP8C00E105R5P2)`，aarch64。没有操作 Mate、实验包、其他设备连接或签名。

首次尝试将本次原生 C 验证程序和库放到新目录 `/data/local/tmp/tantivy-api-v2-20261017-2115`。HDC 对带路径的本地文件保留了末级父目录，首次调用落在目录上；核对实际路径并设置可执行位后，系统仍返回 `Permission denied`。这次 shell 尝试没有运行到 C 测试入口，也没有创建测试索引，不记作成功。后续按正常签名应用路径完成了下面的验证，没有关闭或绕过设备安全限制。

为正常应用调用准备了独立工程：

```powershell
.\prepare_api_smoke.ps1
# 在生成的 build/api-v2-harmony 中：
ohpm install --all
hvigorw --mode module -p product=default -p module=entry@default -p buildMode=debug assembleHap --no-daemon
```

包名 `com.torry2022.tantivyapiprobe`，与原实验包不同。新脚本拒绝覆盖已有目标，避免覆盖后续配置的签名。工程通过 NAPI 调用同一份公开 C 验证程序，报告在自身 filesDir 内。首次未签名构建 **BUILD SUCCESSFUL**。编译保留了模板的 getContext 弃用、可能抛异常和 SDK gcc-toolchain 参数警告，没有为此改动其他业务代码。

已解包检查 `entry-default-unsigned.hap` 包含 `libtantivy_probe.so` 和 `libtantivy_ohos.so`。Hvigor strip 后的包内适配库为 9,767,216 字节，SHA-256 为 `1595be1cbd6d0d2f2d98dd3751a2376be6205beea17323696c4d78660da0fd38`；它与上面构建目录中未经过该 strip 步骤的库分别记录，不能混用哈希。

后续只读查阅“Turbo AI Chat开发主线2”上午记录，确认当时同样遇到 shell 执行限制，改用独立 HAP；用户完成自动签名后才运行。可关联已有调试证书和密钥，无需新增证书，新包仍需匹配它与目标设备的 Profile。没有读取、复制或更改主 Session 的签名配置。

用户完成本包自动签名并明确要求继续 Pura 验证后，核对 Profile、证书和密钥文件存在。生成工程中 `default` 产品尚未关联已生成的 `default` signingConfig，因此只在该工程的本地 `build-profile.json5` 补上关联，重新签名构建通过。签名配置和材料不进入源码或审阅包。未复用 `com.torry2022.retrievalprobe` 的包或签名配置。

### 签名产物与实际运行

安装前确认 Pura 没有本测试包。解包确认签名 HAP 的包名为 `com.torry2022.tantivyapiprobe`，包含上述相同的包内适配库：

```text
entry-default-signed.hap
size: 11313902 bytes
SHA-256: b383480779a55aa96b1c6f29d269eee0d7a5fbbeda8581da5bf1da5d62b9c033
packaged libtantivy_ohos.so SHA-256:
1595be1cbd6d0d2f2d98dd3751a2376be6205beea17323696c4d78660da0fd38
```

实际安装返回 `install bundle successfully`。使用新 runId `api-v2-20261017-pura-01`，索引位于本包 `filesDir/<runId>/index`，没有覆盖已有数据。阶段之间仅停止本测试包自身：

```powershell
hdc -t 10.3.148.94:40013 install <entry-default-signed.hap>
hdc -t 10.3.148.94:40013 shell aa start -a EntryAbility -b com.torry2022.tantivyapiprobe --ps phase create --ps runId api-v2-20261017-pura-01
# 后续每个阶段前执行 aa force-stop com.torry2022.tantivyapiprobe，
# 再分别以 --ps phase reopen / verify / readonly 启动，保持同一 runId。
```

从本包调试沙箱用 `hdc file recv -b com.torry2022.tantivyapiprobe` 取回报告、完整调用日志和错误日志，并逐项检查：

| 阶段 | 进程 PID | 报告 | 错误日志 |
| --- | --- | --- | --- |
| create | 60266 | PASS | 0 字节 |
| reopen | 60888 | PASS | 0 字节 |
| verify | 60934 | PASS | 0 字节 |
| readonly | 61113 | PASS | 0 字节 |

真机实际覆盖：自定义五字段、中文/短语/混合查询、精确字符串大小写、I64/F64 数值范围与类型化返回、三种投影；提交前不可见、commit 后其他 reader 仍需 reload、两个只读句柄与 writer 共存、拒绝第二 writer、只读写入/删除/commit 报错；更新删除跨进程持久化、旧关键词消失、关闭不自动提交；结果在全部句柄关闭后仍可读取。测试源码为 `smoke/tantivy_api_smoke.c`，没有替换为诊断库。

单独 readonly 阶段前后停止本包，分别完整取回索引目录。两次均为 **30 个文件**，逐文件的相对路径、长度和 SHA-256 完全相同，包含隐藏元数据和锁文件。设备比较记录为 `readonly-comparison.json`。这是设备上内容无变化的验证；修改时间无变化的断言另由宿主测试完成，不把本地传输时间当成设备文件修改时间。最终仅停止测试包，保留其测试索引与报告供复核。

## 打包与未覆盖

更新了打包脚本以显式核对审阅后的库哈希、拒绝未提交/未跟踪源码并记录实际 ABI/适配版本。它不再沿用第一版固定哈希或暗示新库经过了旧真机验证。Python 语法和 `--help` 检查通过；调用脚本验证未提交源码拦截，按预期返回 `Review and commit all source changes before packaging`，未生成发布归档。没有提交或发布。

仍未验证：目标设备的并发锁压力和文件回收竞争、大规模/并发压力、资源不足与故障恢复、断电、不同系统和文件系统、跨 Tantivy 版本迁移、性能和相关性指标。本轮通过的是上述版本组合和实际测试范围，不据此宣称全功能或生产稳定性。历史 ABI 1 设备记录单独保留在 [VALIDATION.md](VALIDATION.md)。
