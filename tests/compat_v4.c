/* Compile ONCE with the published ABI 4 header and library. Run the same
 * executable with the candidate library. read modes never create a writer. */
#include "tantivy_ohos.h"
#include <stdio.h>
#include <string.h>
#define TRY(call) do { if ((call) != 0) goto fail; } while (0)
int main(int argc, char** argv) {
    tantivy_ohos_handle* h = NULL;
    tantivy_ohos_result* result = NULL;
    char* error = NULL;
    size_t count = 0;
    uint64_t total = 0;
    bool exact = false;
    tantivy_ohos_hit_view_v2 hit = {0};
    tantivy_ohos_value_view_v2 value = {0};
    const tantivy_ohos_open_options_v2 ro = {
        sizeof(ro), TANTIVY_OHOS_OPTIONS_V2, TANTIVY_OHOS_READ_ONLY
    };
    const tantivy_ohos_field_value_v2 fields[] = {
        {"title", TANTIVY_OHOS_TEXT, "鸿蒙星河检索", 0, 0},
        {"year", TANTIVY_OHOS_I64, NULL, 2026, 0}
    };
    const char* config = "{\"version\":3,\"tokenizer\":\"zh_jieba_v3\","
        "\"fields\":[{\"name\":\"title\",\"kind\":1,\"flags\":15},"
        "{\"name\":\"year\",\"kind\":3,\"flags\":3}],\"fast_fields\":[\"year\"],"
        "\"dictionary\":{\"mode\":\"extend_default\",\"entries\":["
        "{\"word\":\"鸿蒙星河检索\",\"frequency\":1000000000}]}}";
    const char* query = "{\"version\":3,\"query\":\"鸿蒙星河检索\",\"limit\":10,"
        "\"projection\":\"default\",\"track_total\":true}";
    if (argc != 3 || (strcmp(argv[2], "create") && strcmp(argv[2], "read") &&
        strcmp(argv[2], "read-legacy"))) return 2;
    if (tantivy_ohos_abi_version() < 4) return 1;
    if (!strcmp(argv[2], "create")) {
        TRY(tantivy_ohos_create_v3(argv[1], config, &h, &error));
        TRY(tantivy_ohos_upsert_v2(h, "a", fields, 2, &error));
        TRY(tantivy_ohos_commit(h, &error));
        tantivy_ohos_close(h); h = NULL;
    }
    TRY(tantivy_ohos_open_v2(argv[1], &ro, &h, &error));
    if (!strcmp(argv[2], "read-legacy")) {
        query = "{\"version\":3,\"query\":\"legacyterm\",\"limit\":10,"
            "\"projection\":\"default\",\"track_total\":true}";
    }
    TRY(tantivy_ohos_search_v3(h, query, &result, &error));
    TRY(tantivy_ohos_result_count_v2(result, &count, &error));
    TRY(tantivy_ohos_result_total_v4(result, &total, &exact, &error));
    if (count != 1 || total != 1 || !exact) goto fail;
    TRY(tantivy_ohos_result_hit_v2(result, 0, &hit, &error));
    if (strcmp(hit.id, !strcmp(argv[2], "read-legacy") ? "legacy" : "a") ||
        hit.field_count != 1 || !(hit.score > 0)) goto fail;
    TRY(tantivy_ohos_result_value_v2(result, 0, 0, &value, &error));
    if (value.kind != TANTIVY_OHOS_TEXT || !value.string_value ||
        !strstr(value.string_value, "鸿蒙")) goto fail;
    tantivy_ohos_free_result_v2(result); result = NULL;
    if (strcmp(argv[2], "read-legacy")) {
        TRY(tantivy_ohos_search_v3(h,
            "{\"version\":3,\"query\":\"*\",\"limit\":10,\"projection\":\"none\","
            "\"sort\":{\"field\":\"year\",\"order\":\"asc\"},\"track_total\":true}",
            &result, &error));
        TRY(tantivy_ohos_result_hit_v2(result, 0, &hit, &error));
        TRY(tantivy_ohos_result_sort_value_v3(result, 0, &value, &exact, &error));
        if (hit.field_count || hit.score != 0 || !exact ||
            value.kind != TANTIVY_OHOS_I64 || value.i64_value != 2026) goto fail;
    }
    tantivy_ohos_free_result_v2(result); tantivy_ohos_close(h);
    printf("ABI 4 consumer %s: PASS\n", argv[2]);
    return 0;
fail:
    fprintf(stderr, "%s\n", error ? error : "compatibility assertion failed");
    tantivy_ohos_free_string(error); tantivy_ohos_free_result_v2(result);
    tantivy_ohos_close(h);
    return 1;
}
