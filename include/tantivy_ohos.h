#ifndef TANTIVY_OHOS_H
#define TANTIVY_OHOS_H
#include <stdbool.h>
#include <stddef.h>
#include <stdint.h>
#ifdef __cplusplus
extern "C" {
#endif
typedef struct SearchHandle tantivy_ohos_handle;
/* All operations on a handle must be serialized. 0 = success, 1 = error.
 * Inputs are valid NUL-terminated UTF-8; pointers must not alias output slots.
 * Free all returned error/JSON strings with tantivy_ohos_free_string (never free()).
 * Error/JSON output slots are cleared before use; free prior values before reuse.
 * create=true requires a new path with an existing parent. Existing paths are not overwritten.
 * Schema: id (exact string), text (stored, Jieba default mode + lowercase, positions).
 * Query syntax: Tantivy QueryParser; implicit AND, explicit boolean/phrase supported.
 * Upsert/delete become visible only after commit. Commit also reloads the reader.
 * Close does NOT commit pending writes. Only one writer per index is allowed.
 * This is an experimental fixed-schema adapter, not Tantivy's complete API.
 */
uint32_t tantivy_ohos_abi_version(void);
int tantivy_ohos_open(const char* path, bool create, tantivy_ohos_handle** out, char** error);
int tantivy_ohos_upsert(tantivy_ohos_handle* h, const char* id, const char* text, char** error);
int tantivy_ohos_delete(tantivy_ohos_handle* h, const char* id, char** error);
int tantivy_ohos_commit(tantivy_ohos_handle* h, char** error);
int tantivy_ohos_search(tantivy_ohos_handle* h, const char* query, size_t limit, char** out_json, char** error);
void tantivy_ohos_close(tantivy_ohos_handle* h);
void tantivy_ohos_free_string(char* value);
#ifdef __cplusplus
}
#endif
#endif
