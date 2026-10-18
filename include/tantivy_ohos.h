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

/* ABI 4 includes ABI 1/2/3 symbols and preserves their function signatures and id/text
 * behavior. *_v2 layouts below are frozen. Set every options.version to 2 and
 * struct_size to sizeof(the exact options type). Unknown sizes/versions fail;
 * future incompatible layouts get new symbols. No packed structs or C enums
 * appear in the ABI. The library and consumer must target the same platform ABI.
 *
 * All int functions return 0 on success, 1 on error. An optional char** error
 * receives an allocated UTF-8 message. Pointer/count/JSON/handle outputs are
 * cleared before use (even on error). Free prior allocations before reusing an
 * output slot. Inputs must not alias outputs. Error slots may be NULL.
 *
 * All non-NULL pointers must refer to valid, aligned objects of the declared
 * type/length. Strings are NUL-terminated UTF-8, copied during the call. Embedded
 * NUL is not supported. NULL strings and invalid UTF-8 fail. Array pairs use
 * NULL/0 for empty, otherwise a non-NULL pointer and the exact element count.
 * The library cannot validate dangling pointers, unterminated strings, lengths
 * beyond the caller's allocation, aliasing, or double frees.
 *
 * Serialize ALL operations on each handle, including reads, reload and close.
 * A handle may move between threads under caller synchronization. Separate
 * handles may be used concurrently; at most one writer per index. Results are
 * independent of handles/reload/close. Serialize result access and release;
 * results may also move between threads. View pointers are borrowed until the
 * result is freed; never mutate or free them separately.
 *
 * Handles: open/create -> close, exactly once. Results: search_v2/search_v3 ->
 * free_result_v2, exactly once. Owned error/JSON strings: free_string, exactly
 * once. The three release functions accept NULL. No other allocator may free
 * these objects. Unwinding Rust panics are caught at C boundaries; close/free
 * cannot report errors. After an operation reports a panic, close that handle.
 * Process aborts (including allocation failure) and caller UB are not caught.
 *
 * Create requires a NEW directory path with an existing parent. It never
 * overwrites. A failed create may leave a partial directory: inspect it and use
 * a new path. Configuration is immutable and saved as tantivy-ohos.json beside
 * Tantivy's files. Keep the entire index directory together. Reopen validates
 * saved configuration against the actual schema; unsupported metadata fails.
 * Original id/text indexes without this file are recognized automatically.
 *
 * commit persists pending writes and reloads ONLY this handle. Other readers
 * see the latest committed state on open/reload; searches use their last loaded
 * snapshot. reload never commits. close never commits. If commit fails during
 * reload, the commit may already be durable: reopen/reload before retrying.
 * An upsert replaces the WHOLE document with the supplied fields; omitted
 * fields are absent. Delete/upsert on a read-only handle fail, as does commit.
 *
 * READ_ONLY never creates a writer, takes a writer lock, or writes index files.
 * It briefly locks the EXISTING .tantivy-meta.lock with read-only file access
 * while opening/reloading segments, coordinating with writer garbage collection.
 * If that file is absent, read-only open/reload fails without creating it.
 * Multiple readers may coexist with a writer. Do not remove index/lock files
 * while handles are live. Use a local filesystem supporting OS file locks.
 */
uint32_t tantivy_ohos_abi_version(void); /* returns 4; accept >= required ABI; v2 options stay version 2 */

/* ABI 1 compatibility API. Fixed id (exact, stored) and text (Jieba, stored).
 * open(create=true) creates this configuration and opens a writer;
 * open(create=false) opens it read/write. Other configurations are rejected.
 * upsert/search below require this configuration. delete/commit work on either
 * configuration. Search returns [{"id":...,"text":...,"score":...}]. Limit 1..1000.
 * QueryParser uses implicit AND, explicit Boolean and phrase syntax. Jieba uses
 * jieba-rs 0.8.1 default dictionary/mode, HMM, then lowercase (zh_jieba_v1).
 */
int tantivy_ohos_open(const char* path, bool create, tantivy_ohos_handle** out, char** error);
int tantivy_ohos_upsert(tantivy_ohos_handle* h, const char* id, const char* text, char** error);
int tantivy_ohos_delete(tantivy_ohos_handle* h, const char* id, char** error);
int tantivy_ohos_commit(tantivy_ohos_handle* h, char** error);
int tantivy_ohos_search(tantivy_ohos_handle* h, const char* query, size_t limit, char** out_json, char** error);
void tantivy_ohos_close(tantivy_ohos_handle* h);
void tantivy_ohos_free_string(char* value);

#define TANTIVY_OHOS_OPTIONS_V2 2u
#define TANTIVY_OHOS_TEXT 1u       /* Jieba + lowercase, positions for phrases */
#define TANTIVY_OHOS_KEYWORD 2u    /* raw, case-sensitive whole string */
#define TANTIVY_OHOS_I64 3u        /* signed 64-bit integer */
#define TANTIVY_OHOS_F64 4u        /* finite IEEE double */
#define TANTIVY_OHOS_STORED 1u
#define TANTIVY_OHOS_INDEXED 2u
#define TANTIVY_OHOS_DEFAULT_QUERY 4u
#define TANTIVY_OHOS_DEFAULT_RETURN 8u
#define TANTIVY_OHOS_READ_WRITE 1u
#define TANTIVY_OHOS_READ_ONLY 2u
#define TANTIVY_OHOS_PROJECT_DEFAULT 0u
#define TANTIVY_OHOS_PROJECT_NONE 1u
#define TANTIVY_OHOS_PROJECT_FIELDS 2u

/* id is implicit, mandatory, stored and exactly indexed; pass it separately on
 * upsert. Up to 64 ADDITIONAL single-valued fields, names matching
 * [A-Za-z][A-Za-z0-9_]{0,63}, unique and case-sensitive; "id" is reserved.
 * Every field must be stored and/or indexed. DEFAULT_RETURN requires STORED.
 * DEFAULT_QUERY requires an indexed TEXT/KEYWORD. Unknown kinds/flags fail.
 * Numeric fields support field-qualified queries (e.g. year:[2020 TO 2026]);
 * no numeric sorting, aggregation, fast fields or multi-valued fields in v2.
 */
typedef struct {
    const char* name;
    uint32_t kind;
    uint32_t flags;
} tantivy_ohos_field_spec_v2;
typedef struct {
    size_t struct_size;
    uint32_t version;
    const tantivy_ohos_field_spec_v2* fields;
    size_t field_count;
} tantivy_ohos_create_options_v2;
typedef struct {
    size_t struct_size;
    uint32_t version;
    uint32_t mode;
} tantivy_ohos_open_options_v2;

/* Zero-initialize value objects. Set name, kind and only the matching value:
 * TEXT/KEYWORD -> string_value, I64 -> i64_value, F64 -> f64_value.
 * Inactive value slots are ignored. Duplicate/unknown fields, wrong types and
 * non-finite floats fail before queuing any change. Empty id is rejected.
 * Missing fields and empty string values are allowed; no null/multi-value type.
 * The original id/text configuration requires text (possibly empty) even when
 * upsert_v2 is used, preserving compatibility with the legacy JSON search.
 */
typedef struct {
    const char* name;
    uint32_t kind;
    const char* string_value;
    int64_t i64_value;
    double f64_value;
} tantivy_ohos_field_value_v2;

/* Projection is EXPLICITLY selected:
 * DEFAULT + NULL/0 -> saved DEFAULT_RETURN fields (may itself be empty).
 * NONE + NULL/0 -> only id and score, zero returned field values.
 * FIELDS + nonempty array -> named STORED additional fields, in requested order.
 * All modes always include id/score in the hit view; don't put "id" in fields.
 * Missing values are omitted; empty strings remain present. Duplicate/unknown/
 * unstored projections fail. NONE does not copy extra field values into results;
 * Indexes without fast_id still decode the stored document block to obtain id;
 * ABI 4 fast_id indexes use the ID fast column for empty projections.
 *
 * query_fields NULL/0 -> saved DEFAULT_QUERY text/keyword fields.
 * A nonempty array overrides those defaults for this query (indexed text,
 * keyword or id). These are unqualified-query defaults, NOT an access filter:
 * Tantivy's field-qualified syntax can target ANY indexed field. No default
 * query fields means queries must explicitly name fields (or use '*').
 */
typedef struct {
    size_t struct_size;
    uint32_t version;
    uint32_t projection;
    const char* const* fields;
    size_t field_count;
    const char* const* query_fields;
    size_t query_field_count;
} tantivy_ohos_search_options_v2;

typedef struct {
    const char* id; /* borrowed from result */
    float score;
    size_t field_count;
} tantivy_ohos_hit_view_v2;
typedef struct {
    const char* name; /* both strings borrowed from result */
    uint32_t kind;
    const char* string_value; /* NULL for numeric kinds */
    int64_t i64_value;
    double f64_value;
} tantivy_ohos_value_view_v2;

int tantivy_ohos_create_v2(const char* path, const tantivy_ohos_create_options_v2* options,
    tantivy_ohos_handle** out, char** error);
int tantivy_ohos_open_v2(const char* path, const tantivy_ohos_open_options_v2* options,
    tantivy_ohos_handle** out, char** error);
int tantivy_ohos_upsert_v2(tantivy_ohos_handle* h, const char* id,
    const tantivy_ohos_field_value_v2* values, size_t count, char** error);
int tantivy_ohos_reload(tantivy_ohos_handle* h, char** error);
/* Owned JSON describing saved version/tokenizer/fields (id is implicit). */
int tantivy_ohos_config_json_v2(tantivy_ohos_handle* h, char** out_json, char** error);
int tantivy_ohos_search_v2(tantivy_ohos_handle* h, const char* query, size_t limit,
    const tantivy_ohos_search_options_v2* options, tantivy_ohos_result** out, char** error);
/* Iteration uses zero-based indexes; out-of-range access returns an error.
 * A successful zero-hit search still returns a result to free. Results hold at
 * most limit hits (1..1000), no total-hit count or stable tie ordering promised.
 */
int tantivy_ohos_result_count_v2(const tantivy_ohos_result* result, size_t* out, char** error);
int tantivy_ohos_result_hit_v2(const tantivy_ohos_result* result, size_t index,
    tantivy_ohos_hit_view_v2* out, char** error);
int tantivy_ohos_result_value_v2(const tantivy_ohos_result* result, size_t hit, size_t field,
    tantivy_ohos_value_view_v2* out, char** error);
void tantivy_ohos_free_result_v2(tantivy_ohos_result* result);

/* Additive v3 interfaces. All existing ownership/thread/UTF-8/error rules apply.
 * JSON requests use version:3 and reject unknown options. No v2 struct changed.
 * create_v3 accepts at most 2 MiB:
 * {"version":3,"tokenizer":"zh_jieba_v3","fields":[{"name":"title",
 *   "kind":1,"flags":15},{"name":"price","kind":4,"flags":3}],
 *  "fast_fields":["price"],"dictionary":{"mode":"extend_default",
 *   "entries":[{"word":"鸿蒙星河检索","frequency":1000000000}]}}
 * Dictionary EXTENDS the embedded jieba-rs 0.8.1 default dictionary, never replaces
 * it. Omitted dictionary means no extensions. Up to 10000 unique nonempty words,
 * <=384 UTF-8 bytes each, without whitespace/NUL; frequency 1..1000000000,
 * default 100000. Actual entries/frequencies and a corruption-detection fingerprint
 * are stored in tantivy-ohos.json. Missing/invalid snapshots fail on reopen,
 * including read-only open_v2. Every index has its own immutable tokenizer;
 * dictionary changes require a new index. Fingerprint is not authentication.
 * Since ABI 4, new create_v3 indexes default fast_id:true, adding a raw string
 * fast column to id. Set fast_id:false explicitly to omit it. Saved metadata
 * missing fast_id (including ABI 3/A indexes) means false on open; no schema
 * migration or metadata writes occur. v2 creation retains its original schema.
 * v3 fast_fields names must be unique additional
 * KEYWORD/I64/F64 fields. Existing indexes are never upgraded or rewritten.
 */
int tantivy_ohos_create_v3(const char* path, const char* config_json,
    tantivy_ohos_handle** out, char** error);

/* search_v3 JSON <=64 KiB:
 * {"version":3,"query":"*","limit":10,"offset":0,"projection":"none",
 *  "sort":{"field":"price","order":"asc"}}
 * projection = default/none/fields, fields and query_fields arrays have the v2
 * semantics. Omitted sort means relevance/BM25. Field sort uses native TopDocs
 * over ALL query matches, with missing values LAST in either direction.
 * I64 preserves all 64 bits; F64 finite numeric ordering; KEYWORD uses raw UTF-8
 * lexicographic ordering (no locale collation). Ties use ascending DocAddress;
 * no configurable secondary key. Non-relevance hit.score is 0, NOT computed BM25.
 * limit 1..1000, offset+limit <=10000; work/storage grows with offset+limit.
 * Pages refer to a loaded snapshot, not a stable cursor across reload/merge/update.
 * A missing fast field fails even for zero matches. Use existing v2 result getters
 * and free_result_v2. Since ABI 4, an empty effective projection reads IDs from
 * the raw fast id column when configured, without docstore decompression.
 * Old/non-fast-id indexes fall back to stored id; any returned stored field
 * requires docstore decoding. This optimization also applies to search_v2.
 * Optional track_total:true (ABI 4) performs a separate exact Count scan on the
 * same Searcher snapshot. TopDocs scoring stays identical with/without Count;
 * only an explicit request pays for the extra scan. Default false does not run
 * Count. Non-finite returned scores (e.g. boost overflow) fail with a clear error
 * and NULL result, including v2 and legacy JSON searches.
 * Reload can change later calls; separate pages do not pin a shared snapshot.
 */
int tantivy_ohos_search_v3(tantivy_ohos_handle* h, const char* request_json,
    tantivy_ohos_result** out, char** error);
/* Borrowed typed sort value. present=false means missing; inactive slots zero.
 * Returns an error for relevance-sorted results or an invalid hit index.
 */
int tantivy_ohos_result_sort_value_v3(const tantivy_ohos_result* result, size_t hit,
    tantivy_ohos_value_view_v2* out, bool* present, char** error);

/* ABI 4 additive getter; existing result structs/getters stay unchanged.
 * exact=true: total is the complete matching live-document count, not page size,
 * including zero matches and offsets beyond the last hit. exact=false: counting
 * was not requested (including all search_v2 results); total is 0/unknown.
 * The count remains valid after reload or handle close as part of the result.
 */
int tantivy_ohos_result_total_v4(const tantivy_ohos_result* result,
    uint64_t* total, bool* exact, char** error);

/* aggregate_v3 JSON <=64 KiB, 1..16 flat named aggregations:
 * {"version":3,"query":"*","aggregations":{
 *   "groups":{"terms":{"field":"tag","size":10,"segment_size":100}},
 *   "prices":{"stats":{"field":"price"}},
 *   "bands":{"range":{"field":"price","ranges":[{"to":0},
 *     {"from":10,"to":20,"key":"high"}]}}},
 *  "memory_limit_bytes":67108864,"bucket_limit":4096}
 * Response: {"version":3,"aggregations":<official Tantivy results>}, owned UTF-8
 * JSON, free_string. Operates on the ENTIRE loaded query match set, not a page.
 * terms: KEYWORD fast only; size default 10, 1..1024; segment_size default
 * min(size*10,4096), size..4096. Count-descending candidates can be truncated per
 * segment: counts/top terms MAY be approximate. Error bounds and
 * sum_other_doc_count are returned; increasing candidates does not guarantee
 * exactness on arbitrary data. Missing values are ignored, empty strings counted.
 * stats: I64/F64 fast; count of present values, sum/min/max/avg in native f64.
 * I64 stats beyond 2^53 can round; no exact integer-sum promise. Zero matches:
 * count=0, sum=0, min/max/avg=null. Non-finite numeric stats return an error.
 * range: I64/F64 fast; 1..128 nonoverlapping ranges, gaps and unsorted requests
 * allowed. from inclusive, to exclusive; omit for unbounded. I64 endpoints must
 * be JSON INTEGERS within [-2^53,2^53], validated before f64 conversion; outside
 * that range or decimal-form I64 endpoints fail. F64 uses finite binary64 bounds.
 * Only requested buckets are returned in request order; default keys range_0,...
 * or unique caller keys <=64 bytes. Native gap/outer buckets still count toward
 * resource consumption and bucket limits, although omitted from the response.
 * memory_limit_bytes 1..67108864 (default max) is Tantivy's estimated aggregation
 * memory guard, not a process RSS cap. bucket_limit 1..4096 (default max) limits
 * native returned buckets, including gap buckets before filtering; actual limits
 * exceeded return errors. Nested/sub/pipeline/other aggregations are unsupported.
 * Unknown/wrong-type/non-fast fields, invalid ranges/options fail explicitly.
 */
int tantivy_ohos_aggregate_v3(tantivy_ohos_handle* h, const char* request_json,
    char** out_json, char** error);
#ifdef __cplusplus
}
#endif
#endif
