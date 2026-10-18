#include "napi/native_api.h"
#include <cstdio>
#include <filesystem>
#include <fstream>
#include <string>
#include <vector>
extern "C" int TantivyApiSmokeMain(int argc, char** argv);
static std::string Read(napi_env env, napi_value value) {
    size_t size = 0;
    napi_get_value_string_utf8(env, value, nullptr, 0, &size);
    std::vector<char> buffer(size + 1);
    napi_get_value_string_utf8(env, value, buffer.data(), buffer.size(), &size);
    return std::string(buffer.data(), size);
}
static napi_value Run(napi_env env, napi_callback_info info) {
    size_t argc = 2; napi_value args[2];
    napi_get_cb_info(env, info, &argc, args, nullptr, nullptr);
    if (argc != 2) { napi_throw_error(env, nullptr, "root and phase required"); return nullptr; }
    std::string root = Read(env, args[0]), phase = Read(env, args[1]);
    if (phase != "create" && phase != "reopen" && phase != "verify" && phase != "readonly") {
        napi_throw_error(env, nullptr, "invalid phase"); return nullptr;
    }
    std::error_code directoryError;
    std::filesystem::create_directories(root, directoryError);
    if (directoryError) { napi_throw_error(env, nullptr, "cannot create test directory"); return nullptr; }
    std::freopen((root + "/details-api-" + phase + ".log").c_str(), "w", stdout);
    std::freopen((root + "/errors-api-" + phase + ".log").c_str(), "w", stderr);
    std::string path = root + "/index";
    char name[] = "tantivy_api_smoke";
    char* argv[] = {name, path.data(), phase.data()};
    const int code = TantivyApiSmokeMain(3, argv);
    const std::string prefix = "Tantivy API ";
    const std::string result = prefix + phase + ": " + (code == 0 ? "PASS\n" : "FAIL\n");
    std::fflush(stdout); std::fflush(stderr);
    std::ofstream(root + "/report-api-" + phase + ".txt") << result;
    napi_value output;
    napi_create_string_utf8(env, result.c_str(), result.size(), &output);
    return output;
}
static napi_value Init(napi_env env, napi_value exports) {
    napi_property_descriptor property = {"run", nullptr, Run, nullptr, nullptr, nullptr, napi_default, nullptr};
    napi_define_properties(env, exports, 1, &property);
    return exports;
}
static napi_module module = {1, 0, nullptr, Init, "tantivy_probe", nullptr, {0}};
extern "C" __attribute__((constructor)) void Register() { napi_module_register(&module); }
