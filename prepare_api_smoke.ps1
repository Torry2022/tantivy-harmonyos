param(
    [string]$Destination = (Join-Path $PSScriptRoot 'build/api-harmony')
)
$ErrorActionPreference = 'Stop'
$library = "$PSScriptRoot/out/aarch64-unknown-linux-ohos/libtantivy_ohos.so"
if (!(Test-Path -LiteralPath $library)) { throw 'Run build_ohos.ps1 first' }
if (Test-Path -LiteralPath $Destination) { throw 'Destination exists; choose a new directory to preserve its signing/configuration' }
Copy-Item -LiteralPath "$PSScriptRoot/smoke/harmony" -Destination $Destination -Recurse
$app = Join-Path $Destination 'AppScope/app.json5'
(Get-Content -LiteralPath $app -Raw).Replace('com.torry2022.retrievalprobe', 'com.torry2022.tantivyapiprobe') | Set-Content -LiteralPath $app -Encoding utf8
$label = Join-Path $Destination 'AppScope/resources/base/element/string.json'
(Get-Content -LiteralPath $label -Raw).Replace('Retrieval Probe', 'Tantivy API') | Set-Content -LiteralPath $label -Encoding utf8
$page = Join-Path $Destination 'entry/src/main/ets/pages/Index.ets'
(Get-Content -LiteralPath $page -Raw).Replace('Tantivy validation', 'Tantivy API validation') | Set-Content -LiteralPath $page -Encoding utf8
$root = $PSScriptRoot.Replace('\', '/')
$cmake = @'
cmake_minimum_required(VERSION 3.5)
project(tantivy_api_probe C CXX)
set(CMAKE_CXX_STANDARD 17)
set(CMAKE_C_STANDARD 99)
set(PORT_ROOT "@PORT_ROOT@")
add_library(tantivy SHARED IMPORTED)
set_target_properties(tantivy PROPERTIES IMPORTED_LOCATION "${CMAKE_CURRENT_LIST_DIR}/../libs/arm64-v8a/libtantivy_ohos.so")
add_library(tantivy_probe SHARED "${PORT_ROOT}/smoke/api_probe.cpp" "${PORT_ROOT}/smoke/@SMOKE_SOURCE@")
set_source_files_properties("${PORT_ROOT}/smoke/@SMOKE_SOURCE@" PROPERTIES COMPILE_DEFINITIONS "main=TantivyApiSmokeMain")
set_target_properties(tantivy_probe PROPERTIES LIBRARY_OUTPUT_DIRECTORY "${CMAKE_CURRENT_BINARY_DIR}/native-output/${OHOS_ARCH}")
target_include_directories(tantivy_probe PRIVATE "${PORT_ROOT}/include")
target_link_libraries(tantivy_probe PRIVATE tantivy libace_napi.z.so)
'@
$cmake.Replace('@PORT_ROOT@', $root).Replace('@SMOKE_SOURCE@', 'supplement_smoke.c') | Set-Content -LiteralPath (Join-Path $Destination 'entry/src/main/cpp/CMakeLists.txt') -Encoding utf8
$libs = Join-Path $Destination 'entry/src/main/libs/arm64-v8a'
New-Item -ItemType Directory -Force -Path $libs | Out-Null
Copy-Item -LiteralPath $library -Destination $libs
Write-Output "Prepared $Destination (com.torry2022.tantivyapiprobe). Configure its own debug Profile in DevEco."
