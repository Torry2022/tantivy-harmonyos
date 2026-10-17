param(
    [string]$OpenHarmonySdk = $env:DEVECO_SDK_HOME,
    [string]$HostLlvm = (Join-Path $PSScriptRoot '.tools/llvm-mingw-20261006-ucrt-x86_64'),
    [int]$Jobs = 2
)
$ErrorActionPreference = 'Stop'
$Target = 'aarch64-unknown-linux-ohos'
if ([string]::IsNullOrWhiteSpace($OpenHarmonySdk)) { throw 'Pass -OpenHarmonySdk (DevEco openharmony directory)' }
$native = Join-Path (Resolve-Path $OpenHarmonySdk).Path 'native'
if (!(Test-Path -LiteralPath "$native/llvm/bin/clang.exe")) { throw 'OpenHarmony clang not found' }
$saved = @{}
$names = @('PATH','CARGO_HOME','RUSTUP_HOME','CARGO_TARGET_X86_64_PC_WINDOWS_GNU_LINKER',
    'CARGO_ENCODED_RUSTFLAGS',"CARGO_TARGET_$($Target.ToUpper().Replace('-','_'))_LINKER",
    "CC_$($Target.Replace('-','_'))", "CXX_$($Target.Replace('-','_'))", "AR_$($Target.Replace('-','_'))")
foreach ($name in $names) { $saved[$name] = [Environment]::GetEnvironmentVariable($name, 'Process') }
try {
    if (Test-Path "$PSScriptRoot/.tools/rustup") {
        $env:RUSTUP_HOME = "$PSScriptRoot/.tools/rustup"
        $env:CARGO_HOME = "$PSScriptRoot/.tools/cargo"
        $env:PATH = "$env:CARGO_HOME/bin;$env:PATH"
    }
    if (Test-Path "$HostLlvm/bin/llvm-dlltool.exe") {
        $env:PATH = "$HostLlvm/bin;$env:PATH"
        # Rust's bundled GNU linker supplies its matching CRT and libgcc.
        $rustSysroot = (& rustc --print sysroot).Trim()
        $gcc = "$rustSysroot/lib/rustlib/x86_64-pc-windows-gnu/bin/self-contained/x86_64-w64-mingw32-gcc.exe"
        if (Test-Path $gcc) { $env:CARGO_TARGET_X86_64_PC_WINDOWS_GNU_LINKER = $gcc }
    }
    New-Item -ItemType Directory -Force -Path "$PSScriptRoot/build" | Out-Null
    $triple = $Target.Replace('-unknown','')
    $cc = "$PSScriptRoot/build/ohos-cc.cmd"
    $cxx = "$PSScriptRoot/build/ohos-cxx.cmd"
    "@`"$native\llvm\bin\clang.exe`" --target=$triple --sysroot=`"$native\sysroot`" %*" | Set-Content -LiteralPath $cc -Encoding ascii
    "@`"$native\llvm\bin\clang++.exe`" --target=$triple --sysroot=`"$native\sysroot`" %*" | Set-Content -LiteralPath $cxx -Encoding ascii
    [Environment]::SetEnvironmentVariable("CARGO_TARGET_$($Target.ToUpper().Replace('-','_'))_LINKER", $cc, 'Process')
    [Environment]::SetEnvironmentVariable("CC_$($Target.Replace('-','_'))", $cc, 'Process')
    [Environment]::SetEnvironmentVariable("CXX_$($Target.Replace('-','_'))", $cxx, 'Process')
    [Environment]::SetEnvironmentVariable("AR_$($Target.Replace('-','_'))", "$native/llvm/bin/llvm-ar.exe", 'Process')
    Remove-Item Env:CARGO_ENCODED_RUSTFLAGS -ErrorAction SilentlyContinue
    Push-Location $PSScriptRoot
    try {
        & cargo rustc --locked --release --lib --target $Target -j $Jobs -- -C 'link-arg=-Wl,-soname,libtantivy_ohos.so'
        if ($LASTEXITCODE -ne 0) { throw 'Cargo build failed' }
    } finally { Pop-Location }
    $out = "$PSScriptRoot/out/$Target"
    New-Item -ItemType Directory -Force -Path $out | Out-Null
    Copy-Item -LiteralPath "$PSScriptRoot/target/$Target/release/libtantivy_ohos.so", "$PSScriptRoot/include/tantivy_ohos.h" -Destination $out
    Get-FileHash -Algorithm SHA256 -LiteralPath "$out/libtantivy_ohos.so"
} finally {
    foreach ($name in $names) { [Environment]::SetEnvironmentVariable($name, $saved[$name], 'Process') }
}
