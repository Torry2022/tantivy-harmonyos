$ErrorActionPreference = 'Stop'
$library = "$PSScriptRoot/out/aarch64-unknown-linux-ohos/libtantivy_ohos.so"
if (!(Test-Path -LiteralPath $library)) { throw 'Run build_ohos.ps1 first' }
$destination = "$PSScriptRoot/smoke/harmony/entry/src/main/libs/arm64-v8a"
New-Item -ItemType Directory -Force -Path $destination | Out-Null
Copy-Item -LiteralPath $library -Destination $destination
Write-Output 'Open smoke/harmony in DevEco Studio. Configure its own debug Profile; reuse your existing debug certificate.'
