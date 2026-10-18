#ifndef TANTIVY_OHOS_H
#define TANTIVY_OHOS_H
#include <stdbool.h>
#include <stddef.h>
#include <stdint.h>
#ifdef __cplusplus
extern "C" {
#endif

typedef struct SearchHandle tantivy_ohos_handle;
typedef struct SearchResult tantivy_ohos_result;

/* Use the library and header from the same SDK. Callers of older releases must
 * rebuild against this interface; the previous numbered APIs are not exported.
 * All int functions return 0 on success, 1 on error. Optional char** error
 * receives an owned UTF-8 string; free it with tantivy_ohos_free_string.
 * Pointer/count/view/JSON outputs are cleared before use, even on failure.
 * Free an earlier allocation before reusing its output slot. Inputs must not
 * alias outputs. All strings are valid NUL-terminated UTF-8; embedded NUL fails.
 * Non-NULL pointers must be valid, aligned and have the declared array length.
 * Use NULL/0 for empty arrays. Invalid pointers/double free cannot be detected.
 *
 * Serialize ALL calls on a handle, including search, reload and close. Separate
 * handles may run concurrently. One writer per index; multiple readers allowed.
 * Results own their data and survive handle close/reload. Serialize result access
 * and release. Views borrow strings until free_result; never free them separately.
 * create/open -> close; search -> free_result; owned strings -> free_string.
 * Release each allocation exactly once; release functions accept NULL.
 * Unwinding Rust panics are caught at C boundaries: close a handle after a panic
 * error. Process aborts, allocation failure and caller UB are not caught.
 */
#define TANTIVY_OHOS_TEXT 1u       /* Jieba + lowercase, phrase positions */
#define TANTIVY_OHOS_KEYWORD 2u    /* exact, case-sensitive whole string */
#define TANTIVY_OHOS_I64 3u
#define TANTIVY_OHOS_F64 4u        /* finite IEEE double */
#define TANTIVY_OHOS_STORED 1u
#define TANTIVY_OHOS_INDEXED 2u
#define TANTIVY_OHOS_DEFAULT_QUERY 4u
#define TANTIVY_OHOS_DEFAULT_RETURN 8u
#define TANTIVY_OHOS_READ_WRITE 1u
#define TANTIVY_OHOS_READ_ONLY 2u

typedef struct {
    size_t struct_size; /* sizeof(tantivy_ohos_open_options) */
    uint32_t mode;      /* READ_WRITE or READ_ONLY */
} tantivy_ohos_open_options;

/* Runtime settings, never saved in index metadata. NULL selects 1 worker and
 * 15000000 bytes. Explicit values require num_threads 1..8 and TOTAL bytes with
 * 15000000 <= floor(memory_budget_bytes / num_threads) < 4293967295.
 * Zero is invalid; remainder bytes are unused. This is the indexing-worker arena
 * budget, NOT process RSS. Tokenizers, queues, readers and merge workers cost extra.
 * Merge/search thread settings are unchanged. Copied during create/open, applies
 * only to this handle. Invalid values fail before opening/creating files.
 */
typedef struct {
    size_t struct_size; /* sizeof(tantivy_ohos_writer_options) */
    uint32_t num_threads;
    uint64_t memory_budget_bytes;
} tantivy_ohos_writer_options;

/* create: NEW directory, existing parent; never overwrite. A failed create may
 * leave a partial directory. Keep the entire index directory together.
 * config_json <=2 MiB, unknown keys fail. Example:
 * {"fields":[{"name":"title","kind":1,"flags":15},
 *            {"name":"price","kind":4,"flags":3}],"fast_fields":["price"]}
 * id is implicit, exactly indexed and stored; don't declare it. Up to 64 extra
 * single-valued fields, unique names [A-Za-z][A-Za-z0-9_]{0,63}. STORED and/or
 * INDEXED required; DEFAULT_RETURN requires STORED; DEFAULT_QUERY requires
 * indexed TEXT/KEYWORD. fast_fields: unique KEYWORD/I64/F64 additional fields.
 * Optional fast_id defaults true: empty projections read IDs without docstore.
 * Optional dictionary: {"mode":"extend_default","entries":[{"word":"鸿蒙检索",
 * "frequency":1000000000}]}. Extends embedded jieba-rs 0.8.1 default dictionary;
 * <=10000 unique words, <=384 UTF-8 bytes, no whitespace/NUL; frequency 1..1e9,
 * default 100000. The immutable dictionary snapshot is stored with the index.
 * Changing schema/dictionary requires a new index. No request version is needed.
 *
 * open validates saved metadata/schema, with no automatic migration. READ_ONLY
 * requires writer_options=NULL; it creates no writer, takes no writer lock and
 * writes no index files. It briefly locks the EXISTING .tantivy-meta.lock using
 * read-only file access on open/reload, coordinating with writer garbage
 * collection. Missing lock file fails without creation. Use a local filesystem
 * with OS file-lock support. Do not remove index/lock files while handles live.
 */
int tantivy_ohos_create(const char* path, const char* config_json,
    const tantivy_ohos_writer_options* writer_options,
    tantivy_ohos_handle** out, char** error);
int tantivy_ohos_open(const char* path, const tantivy_ohos_open_options* options,
    const tantivy_ohos_writer_options* writer_options,
    tantivy_ohos_handle** out, char** error);

/* Zero-initialize. Set name, kind and its value: TEXT/KEYWORD -> string_value,
 * I64 -> i64_value, F64 -> f64_value. Inactive slots ignored. Unknown/duplicate
 * fields, wrong types, nonfinite floats and empty IDs fail before mutation.
 * Upsert replaces the WHOLE document; omitted fields become absent.
 */
typedef struct {
    const char* name;
    uint32_t kind;
    const char* string_value;
    int64_t i64_value;
    double f64_value;
} tantivy_ohos_field_value;
int tantivy_ohos_upsert(tantivy_ohos_handle* h, const char* id,
    const tantivy_ohos_field_value* values, size_t count, char** error);
int tantivy_ohos_delete(tantivy_ohos_handle* h, const char* id, char** error);
/* commit persists writes/deletes and reloads ONLY this handle. Other readers
 * call reload to see updates. reload and close never commit. A reload failure
 * after commit may mean writes are already durable: reload/reopen to confirm.
 * Mutation and commit fail on read-only handles.
 */
int tantivy_ohos_commit(tantivy_ohos_handle* h, char** error);
int tantivy_ohos_reload(tantivy_ohos_handle* h, char** error);
/* Owned saved index configuration JSON; its format version is index metadata. */
int tantivy_ohos_config_json(tantivy_ohos_handle* h, char** out_json, char** error);

/* request_json <=64 KiB; unknown options fail. Example:
 * {"query":"鸿蒙 AND 检索","limit":10,"offset":0,"projection":"none",
 *  "track_total":true,"sort":{"field":"price","order":"asc"}}
 * query: native QueryParser, implicit AND, Boolean/phrase/field/range syntax.
 * limit 1..1000, offset+limit <=10000; defaults offset=0, projection="default",
 * track_total=false, relevance/BM25 sort. projection: default/none/fields;
 * default uses saved DEFAULT_RETURN fields; none returns ID/score only;
 * fields needs a nonempty array of named STORED fields excluding id. Missing
 * values omitted, empty strings preserved. fields must be empty for other modes.
 * query_fields optionally replaces unqualified-query default fields with indexed
 * TEXT/KEYWORD/id fields. This is NOT an access filter: qualified queries can
 * target any indexed field. With no default fields, use qualified queries or '*'.
 * Field sort uses native TopDocs over all matches; needs a fast field, including
 * zero-match queries. Missing values LAST both directions. I64 preserves 64 bits,
 * F64 finite numeric order, KEYWORD raw UTF-8 order. Ties use DocAddress ascending;
 * no configurable secondary key. Field-sorted score=0, not BM25. Relevance ties
 * have no stable order. Nonfinite scores fail, never return NaN/Infinity.
 * track_total adds a separate Count scan on the same Searcher snapshot without
 * changing TopDocs scores. Separate page calls don't pin a snapshot across reload.
 * Old indexes without fast ID columns fall back to stored IDs; returned stored
 * fields always require docstore decoding. Even zero hits returns a result to free.
 */
int tantivy_ohos_search(tantivy_ohos_handle* h, const char* request_json,
    tantivy_ohos_result** out, char** error);
typedef struct {
    const char* id; /* borrowed */
    float score;
    size_t field_count;
} tantivy_ohos_hit_view;
typedef struct {
    const char* name; /* borrowed */
    uint32_t kind;
    const char* string_value; /* borrowed, NULL for numbers */
    int64_t i64_value;
    double f64_value;
} tantivy_ohos_value_view;
/* Zero-based indexes. count is PAGE length; total is exact only when requested.
 * exact=false/total=0 means not counted. Views remain valid until free_result.
 */
int tantivy_ohos_result_count(const tantivy_ohos_result* result, size_t* out, char** error);
int tantivy_ohos_result_hit(const tantivy_ohos_result* result, size_t index,
    tantivy_ohos_hit_view* out, char** error);
int tantivy_ohos_result_value(const tantivy_ohos_result* result, size_t hit, size_t field,
    tantivy_ohos_value_view* out, char** error);
int tantivy_ohos_result_total(const tantivy_ohos_result* result,
    uint64_t* total, bool* exact, char** error);
/* Field sort only. present=false: missing, inactive slots zero. */
int tantivy_ohos_result_sort_value(const tantivy_ohos_result* result, size_t hit,
    tantivy_ohos_value_view* out, bool* present, char** error);

/* request <=64 KiB: {"query":"*","aggregations":{"prices":{"stats":
 * {"field":"price"}}},"memory_limit_bytes":67108864,"bucket_limit":4096}
 * 1..16 flat named terms/stats/range aggregations over ALL matching docs.
 * terms: KEYWORD fast; size 1..1024 (default10), segment_size size..4096
 * (default min(size*10,4096)); candidates can be approximate. Native error bounds
 * and sum_other_doc_count included; missing ignored, empty strings counted.
 * stats: I64/F64 fast, native f64 sum/min/max/avg; integers beyond 2^53 can round.
 * Zero matches: count=0, sum=0, min/max/avg=null. Nonfinite stats fail.
 * range: I64/F64 fast, 1..128 nonoverlapping ranges, from inclusive/to exclusive;
 * omit for unbounded. Gaps and unsorted ranges allowed. I64 boundaries must be
 * JSON integers in [-2^53,2^53], F64 finite doubles. Returned buckets follow
 * request order; keys default range_0,... or unique strings <=64 bytes.
 * memory_limit_bytes 1..67108864 (default max): native estimated aggregation
 * memory guard, NOT RSS cap. bucket_limit 1..4096 (default max): native gap/outer
 * buckets also consume this budget but are filtered out of returned ranges.
 * Invalid fields/types/options or exceeded limits fail. Nested/sub/pipeline
 * aggregations unsupported. Response: {"aggregations":<native results>}, owned
 * JSON to free_string. See README for examples.
 */
int tantivy_ohos_aggregate(tantivy_ohos_handle* h, const char* request_json,
    char** out_json, char** error);
void tantivy_ohos_free_result(tantivy_ohos_result* result);
void tantivy_ohos_free_string(char* value);
void tantivy_ohos_close(tantivy_ohos_handle* h);
#ifdef __cplusplus
}
#endif
#endif
