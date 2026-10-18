/* C++11: legacy indexing, a v2 read-only handle, and RAII ownership.
 * Run: basic_cpp <new-index-path>. Never removes or overwrites an index.
 */
#include "tantivy_ohos.h"
#include <iostream>
#include <memory>
#include <stdexcept>
#include <string>

using Handle = std::unique_ptr<tantivy_ohos_handle, decltype(&tantivy_ohos_close)>;
using Result = std::unique_ptr<tantivy_ohos_result, decltype(&tantivy_ohos_free_result_v2)>;
using String = std::unique_ptr<char, decltype(&tantivy_ohos_free_string)>;
static void checked(int status, char*& error) {
    String owned(error, tantivy_ohos_free_string);
    error = nullptr;
    if (status) throw std::runtime_error(owned ? owned.get() : "operation failed");
}
int main(int argc, char** argv) {
    if (argc != 2) { std::cerr << "usage: basic_cpp <new-index-path>\n"; return 2; }
    try {
        char* error = nullptr;
        tantivy_ohos_handle* raw = nullptr;
        checked(tantivy_ohos_open(argv[1], true, &raw, &error), error);
        Handle writer(raw, tantivy_ohos_close);
        checked(tantivy_ohos_upsert(writer.get(), "a", "鸿蒙设备支持中文检索", &error), error);
        checked(tantivy_ohos_commit(writer.get(), &error), error);
        writer.reset();
        const tantivy_ohos_open_options_v2 ro = {
            sizeof(ro), TANTIVY_OHOS_OPTIONS_V2, TANTIVY_OHOS_READ_ONLY
        };
        checked(tantivy_ohos_open_v2(argv[1], &ro, &raw, &error), error);
        Handle reader(raw, tantivy_ohos_close);
        const tantivy_ohos_search_options_v2 search = {
            sizeof(search), TANTIVY_OHOS_OPTIONS_V2, TANTIVY_OHOS_PROJECT_NONE,
            nullptr, 0, nullptr, 0
        };
        tantivy_ohos_result* raw_result = nullptr;
        checked(tantivy_ohos_search_v2(reader.get(), "检索", 10, &search, &raw_result, &error), error);
        Result result(raw_result, tantivy_ohos_free_result_v2);
        char* raw_json = nullptr;
        checked(tantivy_ohos_search(reader.get(), "检索", 10, &raw_json, &error), error);
        String json(raw_json, tantivy_ohos_free_string);
        std::cout << json.get() << '\n';
        reader.reset(); // result keeps its own data
        size_t count = 0;
        checked(tantivy_ohos_result_count_v2(result.get(), &count, &error), error);
        for (size_t i = 0; i < count; ++i) {
            tantivy_ohos_hit_view_v2 hit = {};
            checked(tantivy_ohos_result_hit_v2(result.get(), i, &hit, &error), error);
            std::cout << hit.id << " score=" << hit.score << " fields=" << hit.field_count << '\n';
        }
    } catch (const std::exception& e) {
        std::cerr << e.what() << '\n'; return 1;
    }
    return 0;
}
