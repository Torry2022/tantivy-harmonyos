#include "tantivy_ohos.h"
#include <cstdio>
#include <stdexcept>
#include <string>
#include <vector>

static void Check(int code, char* error) {
    std::string message = error ? error : "unknown error";
    tantivy_ohos_free_string(error);
    if (code) throw std::runtime_error(message);
}

static void Query(tantivy_ohos_handle* handle, const char* text, const std::vector<std::string>& ids) {
    char* error = nullptr; char* json = nullptr;
    int code = tantivy_ohos_search(handle, text, 20, &json, &error);
    Check(code, error);
    std::string result = json ? json : "";
    tantivy_ohos_free_string(json);
    size_t count = 0, position = 0;
    while ((position = result.find("\"id\":", position)) != std::string::npos) { ++count; position += 5; }
    bool matched = count == ids.size();
    for (const auto& id : ids) matched = matched && result.find("\"id\":\"" + id + "\"") != std::string::npos;
    std::printf("query=%s result=%s\n", text, result.c_str());
    if (!matched) throw std::runtime_error(std::string("unexpected results: ") + text);
}

static void Upsert(tantivy_ohos_handle* handle, const char* id, const char* text) {
    char* error = nullptr;
    int code = tantivy_ohos_upsert(handle, id, text, &error);
    Check(code, error);
}

int TantivySmokeMain(int argc, char** argv) {
    if (argc != 3) { std::fprintf(stderr, "usage: tantivy_smoke <new-path> <create|reopen|verify>\n"); return 2; }
    const std::string phase = argv[2];
    if (phase != "create" && phase != "reopen" && phase != "verify") return 2;
    tantivy_ohos_handle* handle = nullptr;
    try {
        char* error = nullptr;
        int code = tantivy_ohos_open(argv[1], phase == "create", &handle, &error);
        Check(code, error);
        if (phase == "create") {
            Upsert(handle, "a", "鸿蒙设备支持端侧检索与中文资料。型号Qwen3，内存16GB。");
            Upsert(handle, "b", "鸿蒙设备提供离线语音。");
            Upsert(handle, "c", "咖啡制作指南。");
            Query(handle, "鸿蒙", {});
            code = tantivy_ohos_commit(handle, &error); Check(code, error);
        }
        Query(handle, "鸿蒙", phase == "verify" ? std::vector<std::string>{"a"} : std::vector<std::string>{"a", "b"});
        Query(handle, "检索", phase == "verify" ? std::vector<std::string>{} : std::vector<std::string>{"a"});
        Query(handle, "xyzneverexists", {});
        if (phase != "verify") {
            Query(handle, "Qwen3", {"a"});
            Query(handle, "16GB", {"a"});
            Query(handle, "鸿蒙 AND 检索", {"a"});
            Query(handle, "\"鸿蒙 设备\"", {"a", "b"});
            // Errors must return owned messages instead of crossing the C ABI as panics.
            char* json = nullptr;
            code = tantivy_ohos_search(handle, "\"unterminated", 10, &json, &error);
            bool invalid = code != 0 && error && !json;
            tantivy_ohos_free_string(error); error = nullptr;
            tantivy_ohos_free_string(json);
            if (!invalid) throw std::runtime_error("malformed query accepted");
        }
        if (phase == "reopen") {
            code = tantivy_ohos_delete(handle, "b", &error); Check(code, error);
            Upsert(handle, "a", "鸿蒙设备已经更新文档。");
            code = tantivy_ohos_commit(handle, &error); Check(code, error);
            Query(handle, "鸿蒙", {"a"}); Query(handle, "检索", {});
        }
        tantivy_ohos_close(handle); handle = nullptr;
        std::puts("PASS Tantivy C ABI phase");
        return 0;
    } catch (const std::exception& exception) {
        tantivy_ohos_close(handle);
        std::fprintf(stderr, "FAIL: %s\n", exception.what());
        return 1;
    }
}

#ifndef TANTIVY_SMOKE_NAPI
int main(int argc, char** argv) { return TantivySmokeMain(argc, argv); }
#endif
