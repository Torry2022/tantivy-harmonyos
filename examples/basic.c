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
    const char* config = "{\"fields\":[{\"name\":\"title\",\"kind\":1,\"flags\":15},"
        "{\"name\":\"category\",\"kind\":2,\"flags\":3},"
        "{\"name\":\"year\",\"kind\":3,\"flags\":3},"
        "{\"name\":\"rating\",\"kind\":4,\"flags\":3}]}";
    const tantivy_ohos_open_options ro = {
        sizeof(ro), TANTIVY_OHOS_READ_ONLY
    };
    const tantivy_ohos_field_value values[] = {
        {"title", TANTIVY_OHOS_TEXT, "HarmonyOS 中文检索", 0, 0.0},
        {"category", TANTIVY_OHOS_KEYWORD, "Guide", 0, 0.0},
        {"year", TANTIVY_OHOS_I64, NULL, 2026, 0.0},
        {"rating", TANTIVY_OHOS_F64, NULL, 0, 4.5}
    };
    if (argc != 2) { fprintf(stderr, "usage: basic_c <new-index-path>\n"); return 2; }
    TRY(tantivy_ohos_create(argv[1], config,NULL, &writer, &error));
    TRY(tantivy_ohos_open(argv[1], &ro,NULL, &reader, &error));
    TRY(tantivy_ohos_upsert(writer, "doc-1", values, 4, &error));
    TRY(tantivy_ohos_commit(writer, &error));
    /* Other readers need an explicit reload after this writer commits. */
    TRY(tantivy_ohos_reload(reader, &error));
    TRY(tantivy_ohos_search(reader, "{\"query\":\"检索\",\"limit\":10,\"projection\":\"none\"}", &result, &error));
    TRY(tantivy_ohos_result_count(result, &count, &error));
    if (count != 1) goto fail;
    for (size_t i = 0; i < count; ++i) {
        tantivy_ohos_hit_view hit = {0};
        TRY(tantivy_ohos_result_hit(result, i, &hit, &error));
        if (hit.field_count != 0) goto fail;
        printf("id=%s score=%g fields=%zu\n", hit.id, (double)hit.score, hit.field_count);
    }
    tantivy_ohos_free_result(result); result = NULL;
    TRY(tantivy_ohos_search(reader,
        "{\"query\":\"category:Guide AND year:[2020 TO 2026]\",\"limit\":10,"
        "\"projection\":\"fields\",\"fields\":[\"title\",\"category\",\"year\",\"rating\"]}",
        &result, &error));
    TRY(tantivy_ohos_result_count(result, &count, &error));
    if (count != 1) goto fail;
    for (size_t i = 0; i < count; ++i) {
        tantivy_ohos_hit_view hit = {0};
        TRY(tantivy_ohos_result_hit(result, i, &hit, &error));
        if (hit.field_count != 4) goto fail;
        for (size_t j = 0; j < hit.field_count; ++j) {
            tantivy_ohos_value_view v = {0};
            TRY(tantivy_ohos_result_value(result, i, j, &v, &error));
            if (v.kind == TANTIVY_OHOS_I64) printf("%s=%" PRId64 "\n", v.name, v.i64_value);
            else if (v.kind == TANTIVY_OHOS_F64) printf("%s=%g\n", v.name, v.f64_value);
            else printf("%s=%s\n", v.name, v.string_value);
        }
    }
    tantivy_ohos_free_result(result);
    tantivy_ohos_close(reader); tantivy_ohos_close(writer);
    return 0;
fail:
    fprintf(stderr, "%s\n", error ? error : "operation failed");
    tantivy_ohos_free_string(error); tantivy_ohos_free_result(result);
    tantivy_ohos_close(reader); tantivy_ohos_close(writer);
    return 1;
}
