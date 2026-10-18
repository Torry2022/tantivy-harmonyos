/* C99: create a NEW index directory, index one document, read structured hits.
 * Run: basic_c <new-index-path>. Never removes or overwrites an index.
 */
#include "tantivy_ohos.h"
#include <inttypes.h>
#include <stdio.h>

#define TRY(call) do { if ((call) != 0) goto fail; } while (0)
int main(int argc, char** argv) {
    tantivy_ohos_handle *writer = NULL, *reader = NULL;
    tantivy_ohos_result* result = NULL;
    char* error = NULL;
    size_t count = 0;
    const tantivy_ohos_field_spec_v2 fields[] = {
        {"title", TANTIVY_OHOS_TEXT, TANTIVY_OHOS_STORED | TANTIVY_OHOS_INDEXED |
            TANTIVY_OHOS_DEFAULT_QUERY | TANTIVY_OHOS_DEFAULT_RETURN},
        {"category", TANTIVY_OHOS_KEYWORD, TANTIVY_OHOS_STORED | TANTIVY_OHOS_INDEXED},
        {"year", TANTIVY_OHOS_I64, TANTIVY_OHOS_STORED | TANTIVY_OHOS_INDEXED},
        {"rating", TANTIVY_OHOS_F64, TANTIVY_OHOS_STORED | TANTIVY_OHOS_INDEXED}
    };
    const tantivy_ohos_create_options_v2 create = {
        sizeof(create), TANTIVY_OHOS_OPTIONS_V2, fields, 4
    };
    const tantivy_ohos_open_options_v2 ro = {
        sizeof(ro), TANTIVY_OHOS_OPTIONS_V2, TANTIVY_OHOS_READ_ONLY
    };
    const tantivy_ohos_field_value_v2 values[] = {
        {"title", TANTIVY_OHOS_TEXT, "HarmonyOS 中文检索", 0, 0.0},
        {"category", TANTIVY_OHOS_KEYWORD, "Guide", 0, 0.0},
        {"year", TANTIVY_OHOS_I64, NULL, 2026, 0.0},
        {"rating", TANTIVY_OHOS_F64, NULL, 0, 4.5}
    };
    const char* projected[] = {"title", "category", "year", "rating"};
    tantivy_ohos_search_options_v2 search = {
        sizeof(search), TANTIVY_OHOS_OPTIONS_V2, TANTIVY_OHOS_PROJECT_NONE,
        NULL, 0, NULL, 0
    };
    if (argc != 2) { fprintf(stderr, "usage: basic_c <new-index-path>\n"); return 2; }
    TRY(tantivy_ohos_create_v2(argv[1], &create, &writer, &error));
    TRY(tantivy_ohos_open_v2(argv[1], &ro, &reader, &error));
    TRY(tantivy_ohos_upsert_v2(writer, "doc-1", values, 4, &error));
    TRY(tantivy_ohos_commit(writer, &error));
    /* Other readers need an explicit reload after this writer commits. */
    TRY(tantivy_ohos_reload(reader, &error));
    TRY(tantivy_ohos_search_v2(reader, "检索", 10, &search, &result, &error));
    TRY(tantivy_ohos_result_count_v2(result, &count, &error));
    if (count != 1) goto fail;
    for (size_t i = 0; i < count; ++i) {
        tantivy_ohos_hit_view_v2 hit = {0};
        TRY(tantivy_ohos_result_hit_v2(result, i, &hit, &error));
        if (hit.field_count != 0) goto fail;
        printf("id=%s score=%g fields=%zu\n", hit.id, (double)hit.score, hit.field_count);
    }
    tantivy_ohos_free_result_v2(result); result = NULL;
    search.projection = TANTIVY_OHOS_PROJECT_FIELDS;
    search.fields = projected; search.field_count = 4;
    TRY(tantivy_ohos_search_v2(reader, "category:Guide AND year:[2020 TO 2026]", 10,
        &search, &result, &error));
    TRY(tantivy_ohos_result_count_v2(result, &count, &error));
    if (count != 1) goto fail;
    for (size_t i = 0; i < count; ++i) {
        tantivy_ohos_hit_view_v2 hit = {0};
        TRY(tantivy_ohos_result_hit_v2(result, i, &hit, &error));
        if (hit.field_count != 4) goto fail;
        for (size_t j = 0; j < hit.field_count; ++j) {
            tantivy_ohos_value_view_v2 v = {0};
            TRY(tantivy_ohos_result_value_v2(result, i, j, &v, &error));
            if (v.kind == TANTIVY_OHOS_I64) printf("%s=%" PRId64 "\n", v.name, v.i64_value);
            else if (v.kind == TANTIVY_OHOS_F64) printf("%s=%g\n", v.name, v.f64_value);
            else printf("%s=%s\n", v.name, v.string_value);
        }
    }
    tantivy_ohos_free_result_v2(result);
    tantivy_ohos_close(reader); tantivy_ohos_close(writer);
    return 0;
fail:
    fprintf(stderr, "%s\n", error ? error : "operation failed");
    tantivy_ohos_free_string(error); tantivy_ohos_free_result_v2(result);
    tantivy_ohos_close(reader); tantivy_ohos_close(writer);
    return 1;
}
