/* ABI 1 regression consumer. Compile against the original header/library, create
 * an index, then run the unchanged executable with the ABI 2 library to read it.
 * Usage: legacy_abi <new-or-existing-index> create|read
 */
#include "tantivy_ohos.h"
#include <stdio.h>
#include <string.h>
#define TRY(call) do { if ((call) != 0) goto fail; } while (0)
int main(int argc, char** argv) {
    tantivy_ohos_handle* h = NULL;
    char *json = NULL, *error = NULL;
    int create;
    if (argc != 3 || (strcmp(argv[2], "create") && strcmp(argv[2], "read"))) return 2;
    create = !strcmp(argv[2], "create");
    if (tantivy_ohos_abi_version() < 1) return 1;
    TRY(tantivy_ohos_open(argv[1], create != 0, &h, &error));
    if (create) {
        TRY(tantivy_ohos_upsert(h, "legacy", "鸿蒙设备支持中文检索 legacyterm", &error));
        TRY(tantivy_ohos_commit(h, &error));
        TRY(tantivy_ohos_upsert(h, "uncommitted", "ghostword", &error));
    }
    TRY(tantivy_ohos_search(h, "legacyterm", 10, &json, &error));
    if (!strstr(json, "\"id\":\"legacy\"") || !strstr(json, "\"text\":") || !strstr(json, "\"score\":")) goto fail;
    tantivy_ohos_free_string(json); json = NULL;
    TRY(tantivy_ohos_search(h, "ghostword", 10, &json, &error));
    if (strcmp(json, "[]")) goto fail;
    tantivy_ohos_free_string(json); tantivy_ohos_close(h);
    printf("legacy ABI consumer %s: PASS (runtime ABI %u)\n", argv[2], (unsigned)tantivy_ohos_abi_version());
    return 0;
fail:
    fprintf(stderr, "%s\n", error ? error : "legacy regression assertion failed");
    tantivy_ohos_free_string(error); tantivy_ohos_free_string(json); tantivy_ohos_close(h);
    return 1;
}
