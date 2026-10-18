/* Standalone C99 target/host smoke test. Run each phase in a separate process:
 * tantivy_api_smoke <new-index-path> create|reopen|verify|readonly
 * Creates no app package, never deletes an existing index, and needs no signing.
 */
#include "tantivy_ohos.h"
#include <inttypes.h>
#include <stdio.h>
#include <string.h>

#define CALL(call) do { if ((call) != 0) { fprintf(stderr, "line %d: %s\n", __LINE__, error ? error : "operation failed"); goto fail; } } while (0)
#define EXPECT(test) do { if (!(test)) { fprintf(stderr, "line %d: assertion failed: %s\n", __LINE__, #test); goto fail; } } while (0)
static int put(tantivy_ohos_handle* h, const char* id, const char* title, char** error) {
    const tantivy_ohos_field_value_v2 values[] = {
        {"title", TANTIVY_OHOS_TEXT, title, 0, 0.0},
        {"body", TANTIVY_OHOS_TEXT, "咖啡制作指南", 0, 0.0},
        {"tag", TANTIVY_OHOS_KEYWORD, "Guide-A", 0, 0.0},
        {"year", TANTIVY_OHOS_I64, NULL, 2026, 0.0},
        {"weight", TANTIVY_OHOS_F64, NULL, 0, 4.5}
    };
    return tantivy_ohos_upsert_v2(h, id, values, 5, error);
}
static int query(tantivy_ohos_handle* h, const char* q, const char* expected_ids,
    uint32_t projection, char** error) {
    tantivy_ohos_result* r = NULL;
    size_t count = 0;
    unsigned seen = 0;
    const tantivy_ohos_search_options_v2 o = {sizeof(o), 2, projection, NULL, 0, NULL, 0};
    if (tantivy_ohos_search_v2(h, q, 20, &o, &r, error)) return 1;
    if (tantivy_ohos_result_count_v2(r, &count, error)) goto fail;
    if (count != strlen(expected_ids)) goto fail;
    for (size_t i = 0; i < count; ++i) {
        tantivy_ohos_hit_view_v2 v = {0};
        if (tantivy_ohos_result_hit_v2(r, i, &v, error)) goto fail;
        if (!v.id || strlen(v.id) != 1 || !strchr(expected_ids, v.id[0])) goto fail;
        const unsigned bit = 1u << (unsigned)(strchr(expected_ids, v.id[0]) - expected_ids);
        if (seen & bit) goto fail;
        seen |= bit;
        if (projection == TANTIVY_OHOS_PROJECT_NONE && v.field_count != 0) goto fail;
        if (projection == TANTIVY_OHOS_PROJECT_DEFAULT && v.field_count != 2) goto fail;
    }
    printf("query %s -> %zu hits (projection %u)\n", q, count, (unsigned)projection);
    tantivy_ohos_free_result_v2(r);
    return 0;
fail:
    fprintf(stderr, "unexpected query result: %s\n", q);
    tantivy_ohos_free_result_v2(r);
    return 1;
}
int main(int argc, char** argv) {
    tantivy_ohos_handle *w = NULL, *r1 = NULL, *r2 = NULL, *conflict = NULL;
    tantivy_ohos_result* retained = NULL;
    char* error = NULL;
    char* config = NULL;
    const tantivy_ohos_open_options_v2 rw = {sizeof(rw), 2, TANTIVY_OHOS_READ_WRITE};
    const tantivy_ohos_open_options_v2 ro = {sizeof(ro), 2, TANTIVY_OHOS_READ_ONLY};
    int creating, verifying;
    if (argc != 3) return 2;
    if (!strcmp(argv[2], "readonly")) {
        CALL(tantivy_ohos_open_v2(argv[1], &ro, &r1, &error));
        CALL(tantivy_ohos_open_v2(argv[1], &ro, &r2, &error));
        CALL(query(r1, "鸿蒙", "a", TANTIVY_OHOS_PROJECT_NONE, &error));
        CALL(tantivy_ohos_reload(r2, &error));
        CALL(query(r2, "检索", "", TANTIVY_OHOS_PROJECT_NONE, &error));
        EXPECT(tantivy_ohos_commit(r1, &error) == 1 && error);
        tantivy_ohos_free_string(error); error = NULL;
        tantivy_ohos_close(r1); tantivy_ohos_close(r2);
        puts("PASS Tantivy API v2 readonly");
        return 0;
    }
    creating = !strcmp(argv[2], "create"); verifying = !strcmp(argv[2], "verify");
    if (!creating && !verifying && strcmp(argv[2], "reopen")) return 2;
    EXPECT(tantivy_ohos_abi_version() >= 2);
    if (creating) {
        const tantivy_ohos_field_spec_v2 fields[] = {
            {"title", TANTIVY_OHOS_TEXT, TANTIVY_OHOS_STORED | TANTIVY_OHOS_INDEXED | TANTIVY_OHOS_DEFAULT_QUERY | TANTIVY_OHOS_DEFAULT_RETURN},
            {"body", TANTIVY_OHOS_TEXT, TANTIVY_OHOS_INDEXED},
            {"tag", TANTIVY_OHOS_KEYWORD, TANTIVY_OHOS_STORED | TANTIVY_OHOS_INDEXED | TANTIVY_OHOS_DEFAULT_RETURN},
            {"year", TANTIVY_OHOS_I64, TANTIVY_OHOS_STORED | TANTIVY_OHOS_INDEXED},
            {"weight", TANTIVY_OHOS_F64, TANTIVY_OHOS_STORED | TANTIVY_OHOS_INDEXED}
        };
        const tantivy_ohos_create_options_v2 co = {sizeof(co), 2, fields, 5};
        CALL(tantivy_ohos_create_v2(argv[1], &co, &w, &error));
        CALL(put(w, "a", "鸿蒙设备支持中文检索 Qwen3 16GB", &error));
        CALL(put(w, "b", "鸿蒙设备提供离线语音", &error));
        CALL(put(w, "c", "咖啡制作指南", &error));
        CALL(query(w, "鸿蒙", "", TANTIVY_OHOS_PROJECT_NONE, &error));
        CALL(tantivy_ohos_open_v2(argv[1], &ro, &r1, &error));
        CALL(tantivy_ohos_open_v2(argv[1], &ro, &r2, &error));
        CALL(tantivy_ohos_commit(w, &error));
        CALL(query(r1, "鸿蒙", "", TANTIVY_OHOS_PROJECT_NONE, &error));
        CALL(tantivy_ohos_reload(r1, &error));
        CALL(query(r1, "鸿蒙", "ab", TANTIVY_OHOS_PROJECT_NONE, &error));
        CALL(query(r2, "鸿蒙", "", TANTIVY_OHOS_PROJECT_NONE, &error));
    } else {
        CALL(tantivy_ohos_open_v2(argv[1], &rw, &w, &error));
        CALL(tantivy_ohos_open_v2(argv[1], &ro, &r1, &error));
        CALL(tantivy_ohos_open_v2(argv[1], &ro, &r2, &error));
    }
    EXPECT(tantivy_ohos_open_v2(argv[1], &rw, &conflict, &error) == 1 && !conflict && error);
    tantivy_ohos_free_string(error); error = NULL;
    EXPECT(tantivy_ohos_commit(r1, &error) == 1 && error);
    tantivy_ohos_free_string(error); error = NULL;
    EXPECT(tantivy_ohos_delete(r1, "a", &error) == 1 && error);
    tantivy_ohos_free_string(error); error = NULL;
    EXPECT(put(r1, "a", "更新", &error) == 1 && error);
    tantivy_ohos_free_string(error); error = NULL;
    CALL(tantivy_ohos_config_json_v2(r1, &config, &error));
    EXPECT(strstr(config, "zh_jieba_v1") && strstr(config, "weight"));
    tantivy_ohos_free_string(config); config = NULL;
    CALL(query(w, "鸿蒙", verifying ? "a" : "ab", TANTIVY_OHOS_PROJECT_DEFAULT, &error));
    CALL(query(w, "检索", verifying ? "" : "a", TANTIVY_OHOS_PROJECT_NONE, &error));
    CALL(query(w, "xyzneverexists", "", TANTIVY_OHOS_PROJECT_NONE, &error));
    if (!verifying) {
        CALL(query(w, "\"鸿蒙 设备\"", "ab", TANTIVY_OHOS_PROJECT_NONE, &error));
        CALL(query(w, "Qwen3 AND 16GB", "a", TANTIVY_OHOS_PROJECT_NONE, &error));
        CALL(query(w, "body:咖啡", "abc", TANTIVY_OHOS_PROJECT_NONE, &error));
        CALL(query(w, "tag:guide-a", "", TANTIVY_OHOS_PROJECT_NONE, &error));
        const char* fields[] = {"year", "weight", "title"};
        const tantivy_ohos_search_options_v2 o = {sizeof(o), 2, TANTIVY_OHOS_PROJECT_FIELDS, fields, 3, NULL, 0};
        tantivy_ohos_hit_view_v2 hit = {0}; tantivy_ohos_value_view_v2 value = {0}; size_t count = 0;
        CALL(tantivy_ohos_search_v2(w, "检索 AND year:[2020 TO 2026] AND weight:[4 TO 5]", 10, &o, &retained, &error));
        CALL(tantivy_ohos_result_count_v2(retained, &count, &error)); EXPECT(count == 1);
        CALL(tantivy_ohos_result_hit_v2(retained, 0, &hit, &error)); EXPECT(!strcmp(hit.id, "a") && hit.field_count == 3);
        CALL(tantivy_ohos_result_value_v2(retained, 0, 0, &value, &error)); EXPECT(value.kind == TANTIVY_OHOS_I64 && value.i64_value == 2026 && !value.string_value);
        CALL(tantivy_ohos_result_value_v2(retained, 0, 1, &value, &error)); EXPECT(value.kind == TANTIVY_OHOS_F64 && value.f64_value == 4.5);
        CALL(tantivy_ohos_result_value_v2(retained, 0, 2, &value, &error)); EXPECT(value.kind == TANTIVY_OHOS_TEXT && strstr(value.string_value, "Qwen3"));
        printf("typed projection: year=%" PRId64 " weight=4.5 title=%s\n", (int64_t)2026, value.string_value);
    }
    if (!strcmp(argv[2], "reopen")) {
        CALL(tantivy_ohos_delete(w, "b", &error));
        CALL(put(w, "a", "鸿蒙设备已经更新文档", &error));
        CALL(tantivy_ohos_commit(w, &error));
        CALL(query(w, "鸿蒙", "a", TANTIVY_OHOS_PROJECT_NONE, &error));
        CALL(query(w, "检索", "", TANTIVY_OHOS_PROJECT_NONE, &error));
    }
    CALL(put(w, "d", "ghostword", &error)); /* close must not commit */
    tantivy_ohos_close(w); w = NULL; tantivy_ohos_close(r1); r1 = NULL; tantivy_ohos_close(r2); r2 = NULL;
    if (retained) {
        tantivy_ohos_value_view_v2 view = {0};
        CALL(tantivy_ohos_result_value_v2(retained, 0, 1, &view, &error)); EXPECT(view.f64_value == 4.5);
        tantivy_ohos_free_result_v2(retained); retained = NULL;
    }
    CALL(tantivy_ohos_open_v2(argv[1], &ro, &r1, &error));
    CALL(query(r1, "ghostword", "", TANTIVY_OHOS_PROJECT_NONE, &error));
    tantivy_ohos_close(r1);
    printf("PASS Tantivy API v2 %s\n", argv[2]);
    return 0;
fail:
    tantivy_ohos_free_string(error); tantivy_ohos_free_string(config); tantivy_ohos_free_result_v2(retained);
    tantivy_ohos_close(conflict); tantivy_ohos_close(r2); tantivy_ohos_close(r1); tantivy_ohos_close(w);
    return 1;
}
