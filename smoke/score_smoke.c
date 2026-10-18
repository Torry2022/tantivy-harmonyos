/* Score overflow and finite boost checks through the current public API. */
#include "tantivy_ohos.h"
#include <math.h>
#include <stdio.h>
#include <string.h>
#define SCORE_CALL(x) do { if ((x) != 0) goto score_fail; } while (0)
#define SCORE_CHECK(x) do { if (!(x)) goto score_fail; } while (0)
int main(int argc, char** argv) {
    tantivy_ohos_handle* h = NULL;
    tantivy_ohos_result* result = NULL;
    char* error = NULL;
    float base = 0;
    const char* bad[] = {"alpha^10000000000000000000000000000000000000000",
        "(alpha^100000000000000000000)^100000000000000000000"};
    const tantivy_ohos_open_options options = {sizeof(options), TANTIVY_OHOS_READ_ONLY};
    if (argc != 3) return 2;
    if (!strcmp(argv[2], "create")) {
        const tantivy_ohos_field_value v = {"text", TANTIVY_OHOS_TEXT, "alpha beta", 0, 0.0};
        SCORE_CALL(tantivy_ohos_create(argv[1], "{\"fields\":[{\"name\":\"text\",\"kind\":1,\"flags\":15}]}", NULL, &h, &error));
        SCORE_CALL(tantivy_ohos_upsert(h, "a", &v, 1, &error));
        SCORE_CALL(tantivy_ohos_commit(h, &error));
    } else SCORE_CALL(tantivy_ohos_open(argv[1], &options, NULL, &h, &error));
    for (size_t k = 0; k < 2; ++k) {
        tantivy_ohos_hit_view hit = {NULL, 0, 0};
        SCORE_CALL(tantivy_ohos_search(h, k ? "{\"query\":\"alpha^2\",\"limit\":10}" :
            "{\"query\":\"alpha\",\"limit\":10}", &result, &error));
        SCORE_CALL(tantivy_ohos_result_hit(result, 0, &hit, &error));
        SCORE_CHECK(isfinite(hit.score) && hit.score > 0 && !strcmp(hit.id, "a"));
        if (k) { SCORE_CHECK(hit.score == 2 * base); } else { base = hit.score; }
        tantivy_ohos_free_result(result); result = NULL;
    }
    for (size_t k = 0; k < 2; ++k) {
        for (int count = 0; count < 2; ++count) {
            char request[512];
            snprintf(request, sizeof(request), "{\"query\":\"%s\",\"limit\":10,\"track_total\":%s}", bad[k], count ? "true" : "false");
            SCORE_CHECK(tantivy_ohos_search(h, request, &result, &error) == 1 && result == NULL && error && strstr(error, "non-finite search score"));
            tantivy_ohos_free_string(error); error = NULL;
        }
    }
    tantivy_ohos_close(h);
    printf("PASS finite score boundary %s\n", argv[2]);
    return 0;
score_fail:
    fprintf(stderr, "score boundary failure: %s\n", error ? error : "assertion");
    tantivy_ohos_free_string(error); tantivy_ohos_free_result(result); tantivy_ohos_close(h);
    return 1;
}
