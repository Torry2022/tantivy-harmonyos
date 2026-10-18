/* C99. Run separate processes: writer_options <new-index> create|reopen|readonly.
 * Only this example's index is modified; use a new path for each trial. */
#include "tantivy_ohos.h"
#include <stdio.h>
#include <string.h>
#define WRITER_TRY(call) do { if ((call) != 0) goto fail; } while (0)
#define WRITER_CHECK(condition) do { if (!(condition)) goto fail; } while (0)
static const char* WriterConfig = "{\"tokenizer\":\"zh_jieba_v3\","
    "\"fields\":[{\"name\":\"title\",\"kind\":1,\"flags\":15}]}";
static int WriterPut(tantivy_ohos_handle* h, const char* id, char** error) {
    const tantivy_ohos_field_value value = {
        "title", TANTIVY_OHOS_TEXT, "鸿蒙检索 alpha", 0, 0.0
    };
    return tantivy_ohos_upsert(h, id, &value, 1, error);
}
int main(int argc, char** argv) {
    tantivy_ohos_handle* h = NULL;
    tantivy_ohos_result* result = NULL;
    char *error = NULL, *saved_config = NULL;
    uint64_t total = 0; bool exact = false;
    const tantivy_ohos_writer_options writer = {
        sizeof(writer), 2, 30000000
    };
    const tantivy_ohos_writer_options larger = {
        sizeof(larger), 3, 60000000
    };
    const tantivy_ohos_open_options rw = {
        sizeof(rw), TANTIVY_OHOS_READ_WRITE
    };
    const tantivy_ohos_open_options ro = {
        sizeof(ro), TANTIVY_OHOS_READ_ONLY
    };
    if (argc != 3 || (strcmp(argv[2], "create") && strcmp(argv[2], "reopen") &&
        strcmp(argv[2], "readonly"))) return 2;
    if (!strcmp(argv[2], "create")) {
        tantivy_ohos_writer_options invalid = writer;
        invalid.num_threads = 0;
        WRITER_CHECK(tantivy_ohos_create(argv[1], WriterConfig, &invalid, &h, &error) == 1);
        WRITER_CHECK(!h && error);
        tantivy_ohos_free_string(error); error = NULL;
        /* Invalid options must not leave a directory that blocks valid create. */
        WRITER_TRY(tantivy_ohos_create(argv[1], WriterConfig, &writer, &h, &error));
        WRITER_TRY(WriterPut(h, "a", &error)); WRITER_TRY(WriterPut(h, "temporary", &error));
        WRITER_TRY(tantivy_ohos_commit(h, &error));
        tantivy_ohos_close(h); h = NULL;
    } else if (!strcmp(argv[2], "reopen")) {
        WRITER_TRY(tantivy_ohos_open(argv[1], &rw, &larger, &h, &error));
        WRITER_TRY(tantivy_ohos_delete(h, "temporary", &error)); WRITER_TRY(WriterPut(h, "b", &error));
        WRITER_TRY(tantivy_ohos_commit(h, &error));
        tantivy_ohos_close(h); h = NULL;
    }
    WRITER_CHECK(tantivy_ohos_open(argv[1], &ro, &writer, &h, &error) == 1);
    WRITER_CHECK(!h && error);
    tantivy_ohos_free_string(error); error = NULL;
    WRITER_TRY(tantivy_ohos_open(argv[1], &ro, NULL, &h, &error));
    WRITER_TRY(tantivy_ohos_search(h,
        "{\"query\":\"alpha\",\"limit\":10,"
        "\"projection\":\"none\",\"track_total\":true}", &result, &error));
    WRITER_TRY(tantivy_ohos_result_total(result, &total, &exact, &error));
    WRITER_CHECK(exact && total == 2);
    WRITER_TRY(tantivy_ohos_config_json(h, &saved_config, &error));
    WRITER_CHECK(!strstr(saved_config, "memory_budget") && !strstr(saved_config, "num_threads"));
    tantivy_ohos_free_string(saved_config); saved_config = NULL;
    tantivy_ohos_free_result(result); result = NULL;
    tantivy_ohos_close(h); h = NULL;
    printf("writer options %s: PASS\n", argv[2]);
    return 0;
fail:
    fprintf(stderr, "%s\n", error ? error : "writer options assertion failed");
    tantivy_ohos_free_string(error); tantivy_ohos_free_string(saved_config);
    tantivy_ohos_free_result(result); tantivy_ohos_close(h);
    return 1;
}
