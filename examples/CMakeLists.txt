cmake_minimum_required(VERSION 3.20)
project(tantivy_consumer LANGUAGES C CXX)

set(TANTIVY_SDK_ROOT "" CACHE PATH "Extracted SDK directory")
option(TANTIVY_WARNINGS_AS_ERRORS "Treat example compiler warnings as errors" OFF)
find_path(TANTIVY_INCLUDE_DIR tantivy_ohos.h
    PATHS "${TANTIVY_SDK_ROOT}/include" NO_DEFAULT_PATH NO_CMAKE_FIND_ROOT_PATH REQUIRED)
find_library(TANTIVY_LIBRARY NAMES tantivy_ohos
    PATHS "${TANTIVY_SDK_ROOT}/lib/arm64-v8a" "${TANTIVY_SDK_ROOT}/lib"
    NO_DEFAULT_PATH NO_CMAKE_FIND_ROOT_PATH REQUIRED)
add_library(tantivy_sdk UNKNOWN IMPORTED)
set_target_properties(tantivy_sdk PROPERTIES
    IMPORTED_LOCATION "${TANTIVY_LIBRARY}"
    INTERFACE_INCLUDE_DIRECTORIES "${TANTIVY_INCLUDE_DIR}")

foreach(example basic_c basic_cpp advanced)
    if(example STREQUAL "basic_c")
        set(source basic.c)
    elseif(example STREQUAL "basic_cpp")
        set(source basic.cpp)
    else()
        set(source advanced.cpp)
    endif()
    add_executable(${example} ${source})
    target_link_libraries(${example} PRIVATE tantivy_sdk)
    set_target_properties(${example} PROPERTIES
        C_STANDARD 99 C_STANDARD_REQUIRED YES C_EXTENSIONS NO
        CXX_STANDARD 11 CXX_STANDARD_REQUIRED YES CXX_EXTENSIONS NO)
    if(CMAKE_C_COMPILER_ID MATCHES "GNU|Clang")
        target_compile_options(${example} PRIVATE -Wall -Wextra)
        if(TANTIVY_WARNINGS_AS_ERRORS)
            target_compile_options(${example} PRIVATE -Werror)
        endif()
    endif()
endforeach()
