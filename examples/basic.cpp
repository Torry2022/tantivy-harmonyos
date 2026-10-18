/* C++11: indexing, read-only query and RAII ownership.
 * Run: basic_cpp <new-index-path>. Never removes or overwrites an index.
 */
#include "tantivy_ohos.h"
#include <iostream>
#include <memory>
#include <stdexcept>
#include <string>

using Handle = std::unique_ptr<tantivy_ohos_handle, decltype(&tantivy_ohos_close)>;
using Result = std::unique_ptr<tantivy_ohos_result, decltype(&tantivy_ohos_free_result)>;
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
        checked(tantivy_ohos_create(argv[1], R"({"fields":[{"name":"text","kind":1,"flags":15}]})", nullptr, &raw, &error), error);
        Handle writer(raw, tantivy_ohos_close);
        const tantivy_ohos_field_value value = {"text", TANTIVY_OHOS_TEXT, "鸿蒙设备支持中文检索", 0, 0.0};
        checked(tantivy_ohos_upsert(writer.get(), "a", &value, 1, &error), error);
        checked(tantivy_ohos_commit(writer.get(), &error), error);
        writer.reset();
        const tantivy_ohos_open_options ro = {
            sizeof(ro), TANTIVY_OHOS_READ_ONLY
        };
        checked(tantivy_ohos_open(argv[1], &ro, nullptr, &raw, &error), error);
        Handle reader(raw, tantivy_ohos_close);
        tantivy_ohos_result* raw_result = nullptr;
        checked(tantivy_ohos_search(reader.get(), R"({"query":"检索","limit":10,"projection":"none"})", &raw_result, &error), error);
        Result result(raw_result, tantivy_ohos_free_result);
        reader.reset(); // result keeps its own data
        size_t count = 0;
        checked(tantivy_ohos_result_count(result.get(), &count, &error), error);
        if (count != 1) throw std::runtime_error("expected one persisted hit");
        for (size_t i = 0; i < count; ++i) {
            tantivy_ohos_hit_view hit = {};
            checked(tantivy_ohos_result_hit(result.get(), i, &hit, &error), error);
            std::cout << hit.id << " score=" << hit.score << " fields=" << hit.field_count << '\n';
        }
    } catch (const std::exception& e) {
        std::cerr << e.what() << '\n'; return 1;
    }
    return 0;
}
