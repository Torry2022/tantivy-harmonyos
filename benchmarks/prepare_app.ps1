param(
    [Parameter(Mandatory=$true)][string]$Destination,
    [Parameter(Mandatory=$true)][string]$Library,
    [Parameter(Mandatory=$true)][string]$HeaderDirectory,
    [switch]$BaselineAbi2
)
$ErrorActionPreference = 'Stop'
$root = (Resolve-Path "$PSScriptRoot/..").Path
if (Test-Path -LiteralPath $Destination) { throw 'Destination exists; preserve its signing and configuration' }
$libraryPath = (Resolve-Path -LiteralPath $Library).Path
$headerPath = (Resolve-Path -LiteralPath $HeaderDirectory).Path.Replace('\','/')
& "$root/prepare_api_smoke.ps1" -Destination $Destination -ApiVersion 4
Copy-Item -LiteralPath $libraryPath -Destination "$Destination/entry/src/main/libs/arm64-v8a/libtantivy_ohos.so"
Copy-Item -LiteralPath "$PSScriptRoot/Index.ets" -Destination "$Destination/entry/src/main/ets/pages/Index.ets"
$cmake = @'
cmake_minimum_required(VERSION 3.5)
project(sdk_benchmark CXX)
set(CMAKE_CXX_STANDARD 17)
add_library(tantivy SHARED IMPORTED)
set_target_properties(tantivy PROPERTIES IMPORTED_LOCATION "${CMAKE_CURRENT_LIST_DIR}/../libs/arm64-v8a/libtantivy_ohos.so")
add_library(tantivy_probe SHARED "@ROOT@/smoke/benchmark_probe.cpp" "@ROOT@/benchmarks/sdk_bench.cpp")
set_source_files_properties("@ROOT@/benchmarks/sdk_bench.cpp" PROPERTIES COMPILE_DEFINITIONS "main=TantivyBenchmarkMain")
@BASELINE@
target_include_directories(tantivy_probe PRIVATE "@HEADER@")
set_target_properties(tantivy_probe PROPERTIES LIBRARY_OUTPUT_DIRECTORY "${CMAKE_CURRENT_BINARY_DIR}/native-output/${OHOS_ARCH}")
target_link_libraries(tantivy_probe PRIVATE tantivy libace_napi.z.so)
'@
$define = if ($BaselineAbi2) { 'target_compile_definitions(tantivy_probe PRIVATE SDK_BASELINE_ABI2)' } else { '' }
$cmake.Replace('@ROOT@',$root.Replace('\','/')).Replace('@HEADER@',$headerPath).Replace('@BASELINE@',$define) | Set-Content -LiteralPath "$Destination/entry/src/main/cpp/CMakeLists.txt" -Encoding utf8
Write-Output 'Prepared own-bundle benchmark project only; no build, installation or device operation.'
