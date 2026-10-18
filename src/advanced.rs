//! Additive v3 functions. The v2 C layouts remain frozen.
use super::*;
use api::{StoredValue, ValueViewV2};
use config::*;
use serde::Deserialize;
use serde_json::{json, Value as Json};
use tantivy::aggregation::{
    agg_req::Aggregations, AggContextParams, AggregationCollector, AggregationLimitsGuard,
};
use tantivy::{DocAddress, Order};

fn decode<T: serde::de::DeserializeOwned>(s: &str, max: usize) -> Result<T> {
    if s.len() > max {
        return Err(format!("JSON request exceeds {max} bytes"));
    }
    serde_json::from_str(s).map_err(|e| e.to_string())
}
fn cstring(s: &str) -> Result<CString> {
    CString::new(s).map_err(|e| e.to_string())
}

/// # Safety
/// Same ownership, serialization and pointer rules as v2; config_json is UTF-8.
#[no_mangle]
pub unsafe extern "C" fn tantivy_ohos_create_v3(
    path: *const c_char,
    config_json: *const c_char,
    out: *mut *mut SearchHandle,
    error: *mut *mut c_char,
) -> i32 {
    if !out.is_null() {
        *out = ptr::null_mut();
    }
    boundary(error, || {
        if out.is_null() {
            return Err("null output handle".into());
        }
        let mut config: Config = decode(string(config_json)?, 2_097_152)?;
        if config.version != 3 {
            return Err("create_v3 requires config version 3".into());
        }
        // Creation defaults are intentionally separate from metadata loading.
        config.fast_id = Some(config.fast_id.unwrap_or(true));
        if config.dictionary.is_none() {
            config.dictionary = Some(Dictionary {
                mode: "extend_default".into(),
                entries: Vec::new(),
            });
        }
        let fingerprint = config.fingerprint()?;
        if config
            .dictionary_fingerprint
            .as_ref()
            .is_some_and(|f| *f != fingerprint)
        {
            return Err("dictionary fingerprint mismatch".into());
        }
        config.dictionary_fingerprint = Some(fingerprint);
        *out = Box::into_raw(Box::new(SearchHandle::open_config(
            string(path)?,
            Some(config),
            false,
        )?));
        Ok(())
    })
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct SearchRequest {
    version: u32,
    query: String,
    limit: usize,
    #[serde(default)]
    offset: usize,
    #[serde(default = "default_projection")]
    projection: String,
    #[serde(default)]
    fields: Vec<String>,
    #[serde(default)]
    query_fields: Vec<String>,
    #[serde(default)]
    sort: Option<Sort>,
    #[serde(default)]
    track_total: bool,
}
fn default_projection() -> String {
    "default".into()
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Sort {
    field: String,
    order: String,
}

impl SearchHandle {
    fn fast_spec(&self, name: &str) -> Result<&FieldConfig> {
        let spec = self
            .config
            .fields
            .iter()
            .find(|f| f.name == name)
            .ok_or_else(|| format!("unknown field: {name}"))?;
        if !self.config.fast_fields.iter().any(|s| s == name) {
            return Err(format!("field {name} has no fast field; create a new index with v3 fast_fields (no in-place migration)"));
        }
        Ok(spec)
    }
    fn advanced_search(&self, request: SearchRequest) -> Result<SearchResult> {
        let r = request;
        if r.version != 3
            || !(1..=1000).contains(&r.limit)
            || r.offset.checked_add(r.limit).is_none_or(|n| n > 10_000)
        {
            return Err("search requires version 3, limit 1..1000 and offset+limit <=10000".into());
        }
        let fields = r
            .fields
            .iter()
            .map(|s| cstring(s))
            .collect::<Result<Vec<_>>>()?;
        let queries = r
            .query_fields
            .iter()
            .map(|s| cstring(s))
            .collect::<Result<Vec<_>>>()?;
        let fp: Vec<_> = fields.iter().map(|s| s.as_ptr()).collect();
        let qp: Vec<_> = queries.iter().map(|s| s.as_ptr()).collect();
        let options = SearchOptionsV2 {
            struct_size: std::mem::size_of::<SearchOptionsV2>(),
            version: 2,
            projection: match r.projection.as_str() {
                "default" => 0,
                "none" => 1,
                "fields" => 2,
                _ => return Err("unknown projection".into()),
            },
            fields: if fp.is_empty() {
                ptr::null()
            } else {
                fp.as_ptr()
            },
            field_count: fp.len(),
            query_fields: if qp.is_empty() {
                ptr::null()
            } else {
                qp.as_ptr()
            },
            query_field_count: qp.len(),
        };
        // JSON strings can contain NUL even though the outer C string cannot.
        cstring(&r.query)?;
        let (query, projected) = unsafe { self.prepare_search(&r.query, &options)? };
        let searcher = self.reader.searcher();
        let top = TopDocs::with_limit(r.limit).and_offset(r.offset);
        if let Some(sort) = r.sort {
            let spec = self.fast_spec(&sort.field)?;
            let order = match sort.order.as_str() {
                "asc" => Order::Asc,
                "desc" => Order::Desc,
                _ => return Err("sort order must be asc or desc".into()),
            };
            let mut total = None;
            let sorted: Vec<(Option<StoredValue>, DocAddress)> = match spec.kind {
                I64 => collect_page(
                    &searcher,
                    query.as_ref(),
                    top.order_by_fast_field::<i64>(&sort.field, order),
                    r.track_total,
                    &mut total,
                )?
                .into_iter()
                .map(|(n, a)| (n.map(StoredValue::I64), a))
                .collect(),
                F64 => collect_page(
                    &searcher,
                    query.as_ref(),
                    top.order_by_fast_field::<f64>(&sort.field, order),
                    r.track_total,
                    &mut total,
                )?
                .into_iter()
                .map(|(n, a)| (n.map(StoredValue::F64), a))
                .collect(),
                KEYWORD => collect_page(
                    &searcher,
                    query.as_ref(),
                    top.order_by_string_fast_field(&sort.field, order),
                    r.track_total,
                    &mut total,
                )?
                .into_iter()
                .map(|(s, a)| {
                    Ok((
                        s.map(|s| cstring(&s).map(StoredValue::String))
                            .transpose()?,
                        a,
                    ))
                })
                .collect::<Result<_>>()?,
                _ => return Err("sorting requires KEYWORD/I64/F64".into()),
            };
            let docs = sorted.iter().map(|(_, a)| (0.0, *a)).collect();
            let mut result = self.materialize(&searcher, docs, projected)?;
            result.total = total;
            for (hit, (value, _)) in result.hits.iter_mut().zip(sorted) {
                hit.sort = Some((cstring(&sort.field)?, spec.kind, value));
            }
            Ok(result)
        } else {
            let mut total = None;
            let docs = collect_page(
                &searcher,
                query.as_ref(),
                top.order_by_score(),
                r.track_total,
                &mut total,
            )?;
            let mut result = self.materialize(&searcher, docs, projected)?;
            result.total = total;
            Ok(result)
        }
    }
}

fn collect_page<C: tantivy::collector::Collector>(
    searcher: &tantivy::Searcher,
    query: &dyn tantivy::query::Query,
    collector: C,
    track_total: bool,
    total: &mut Option<u64>,
) -> Result<C::Fruit> {
    // A tuple collector bypasses TopDocs' specialized pruning traversal and can
    // change disjunction f32 accumulation/near-tie order. Keep TopDocs identical;
    // count only when requested, with a separate scan of this pinned Searcher.
    let page = searcher
        .search(query, &collector)
        .map_err(|e| e.to_string())?;
    if track_total {
        let count = searcher
            .search(query, &tantivy::collector::Count)
            .map_err(|e| e.to_string())?;
        *total = Some(count as u64);
    }
    Ok(page)
}

/// # Safety
/// See v2 result ownership and pointer rules. exact=false means not requested.
#[no_mangle]
pub unsafe extern "C" fn tantivy_ohos_result_total_v4(
    result: *const SearchResult,
    total: *mut u64,
    exact: *mut bool,
    error: *mut *mut c_char,
) -> i32 {
    if !total.is_null() {
        *total = 0;
    }
    if !exact.is_null() {
        *exact = false;
    }
    boundary(error, || {
        if total.is_null() || exact.is_null() {
            return Err("null total output".into());
        }
        let r = result.as_ref().ok_or("null result")?;
        if let Some(n) = r.total {
            *total = n;
            *exact = true;
        }
        Ok(())
    })
}

/// # Safety
/// See v2 pointer and ownership contract. Returns the existing opaque result type.
#[no_mangle]
pub unsafe extern "C" fn tantivy_ohos_search_v3(
    h: *mut SearchHandle,
    request_json: *const c_char,
    out: *mut *mut SearchResult,
    error: *mut *mut c_char,
) -> i32 {
    if !out.is_null() {
        *out = ptr::null_mut();
    }
    boundary(error, || {
        if out.is_null() {
            return Err("null result output".into());
        }
        let request = decode(string(request_json)?, 65_536)?;
        *out = Box::into_raw(Box::new(handle(h)?.advanced_search(request)?));
        Ok(())
    })
}

/// # Safety
/// Borrowed value strings live until free_result_v2. A relevance result has no sort value.
#[no_mangle]
pub unsafe extern "C" fn tantivy_ohos_result_sort_value_v3(
    result: *const SearchResult,
    hit: usize,
    out: *mut ValueViewV2,
    present: *mut bool,
    error: *mut *mut c_char,
) -> i32 {
    if !out.is_null() {
        *out = ValueViewV2::default();
    }
    if !present.is_null() {
        *present = false;
    }
    boundary(error, || {
        if out.is_null() || present.is_null() {
            return Err("null sort output".into());
        }
        let r = result.as_ref().ok_or("null result")?;
        let h = r.hits.get(hit).ok_or("hit index out of range")?;
        let (name, kind, value) = h
            .sort
            .as_ref()
            .ok_or("relevance result has no field sort value")?;
        let mut v = ValueViewV2 {
            name: name.as_ptr(),
            kind: *kind,
            ..Default::default()
        };
        if let Some(value) = value {
            match value {
                StoredValue::String(s) => v.string_value = s.as_ptr(),
                StoredValue::I64(n) => v.i64_value = *n,
                StoredValue::F64(n) => v.f64_value = *n,
            }
        }
        *present = value.is_some();
        *out = v;
        Ok(())
    })
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct AggregateRequest {
    version: u32,
    query: String,
    aggregations: serde_json::Map<String, Json>,
    #[serde(default = "default_memory")]
    memory_limit_bytes: u64,
    #[serde(default = "default_buckets")]
    bucket_limit: u32,
}
fn default_memory() -> u64 {
    64 * 1024 * 1024
}
fn default_buckets() -> u32 {
    4096
}
fn object_keys<'a>(value: &'a Json, keys: &[&str]) -> Result<&'a serde_json::Map<String, Json>> {
    let map = value.as_object().ok_or("expected JSON object")?;
    if map.keys().any(|k| !keys.contains(&k.as_str())) {
        return Err("unsupported aggregation option or nested aggregation".into());
    }
    Ok(map)
}
fn positive(v: Option<&Json>, default: u64, max: u64) -> Result<u64> {
    let n = match v {
        None => default,
        Some(v) => v.as_u64().ok_or("expected positive integer")?,
    };
    if n == 0 || n > max {
        return Err(format!("resource limit: integer must be 1..{max}"));
    }
    Ok(n)
}
impl SearchHandle {
    fn aggregate(&self, mut r: AggregateRequest) -> Result<String> {
        if r.version != 3 || r.aggregations.is_empty() || r.aggregations.len() > 16 {
            return Err("aggregation requires version 3 and 1..16 flat named aggregations".into());
        }
        if r.memory_limit_bytes == 0
            || r.memory_limit_bytes > default_memory()
            || r.bucket_limit == 0
            || r.bucket_limit > 4096
        {
            return Err("resource limit: memory 1..67108864 bytes; buckets 1..4096".into());
        }
        let mut range_projections =
            std::collections::BTreeMap::<String, Vec<(String, String)>>::new();
        let mut stats_names = Vec::new();
        for (name, agg) in &mut r.aggregations {
            if name.is_empty() || name.len() > 64 || name.contains('\0') {
                return Err("invalid aggregation name".into());
            }
            let map = object_keys(agg, &["terms", "stats", "range"])?;
            if map.len() != 1 {
                return Err("each aggregation must have exactly one type".into());
            }
            let (kind, body) = map.iter().next().unwrap();
            let allowed = match kind.as_str() {
                "terms" => vec!["field", "size", "segment_size"],
                "stats" => vec!["field"],
                "range" => vec!["field", "ranges"],
                _ => unreachable!(),
            };
            let opts = object_keys(body, &allowed)?;
            let field = opts
                .get("field")
                .and_then(Json::as_str)
                .ok_or("aggregation requires field")?;
            let spec = self.fast_spec(field)?;
            match kind.as_str() {
                "terms" => {
                    if spec.kind != KEYWORD {
                        return Err("terms requires KEYWORD fast field".into());
                    }
                    let size = positive(opts.get("size"), 10, 1024)?;
                    let segment = positive(
                        opts.get("segment_size"),
                        size.saturating_mul(10).min(4096),
                        4096,
                    )?;
                    if segment < size {
                        return Err("segment_size must be >= size".into());
                    }
                    *agg = json!({"terms":{"field":field,"size":size,"segment_size":segment,"show_term_doc_count_error":true}});
                }
                "stats" => {
                    stats_names.push(name.clone());
                    if !matches!(spec.kind, I64 | F64) {
                        return Err("stats requires numeric fast field".into());
                    }
                }
                "range" => {
                    if !matches!(spec.kind, I64 | F64) {
                        return Err("range requires numeric fast field".into());
                    }
                    let ranges = opts
                        .get("ranges")
                        .and_then(Json::as_array)
                        .ok_or("range requires ranges array")?;
                    if ranges.is_empty() || ranges.len() > 128 {
                        return Err("range requires 1..128 buckets".into());
                    }
                    let mut intervals = Vec::new();
                    let mut keys = std::collections::HashSet::new();
                    let mut normalized = Vec::new();
                    let mut projection = Vec::new();
                    for (i, range) in ranges.iter().enumerate() {
                        let opts = object_keys(range, &["from", "to", "key"])?;
                        let endpoint = |key: &str| -> Result<Option<f64>> {
                            match opts.get(key) {
                                None => Ok(None),
                                Some(v) if spec.kind == I64 => {
                                    // Inspect the JSON integer BEFORE any lossy f64 conversion.
                                    let n = v.as_i64().ok_or("I64 range endpoints must be JSON integers within [-2^53,2^53]")?;
                                    if !(-9_007_199_254_740_992..=9_007_199_254_740_992)
                                        .contains(&n)
                                    {
                                        return Err("I64 range endpoint exceeds exact f64 integer range [-2^53,2^53]".into());
                                    }
                                    Ok(Some(n as f64))
                                }
                                Some(v) => {
                                    v.as_f64().filter(|v| v.is_finite()).map(Some).ok_or_else(
                                        || "range endpoints must be finite numbers".into(),
                                    )
                                }
                            }
                        };
                        let from = endpoint("from")?;
                        let to = endpoint("to")?;
                        let a = from.unwrap_or(f64::NEG_INFINITY);
                        let b = to.unwrap_or(f64::INFINITY);
                        if a >= b {
                            return Err("range requires from < to".into());
                        }
                        intervals.push((a, b));
                        let key = match opts.get("key") {
                            None => format!("range_{i}"),
                            Some(k) => k.as_str().ok_or("range key must be string")?.to_owned(),
                        };
                        if key.len() > 64 || key.contains('\0') || !keys.insert(key.clone()) {
                            return Err("invalid/duplicate range key".into());
                        }
                        // Tantivy fills gaps and outer ranges; keep internal keys distinct so
                        // those buckets can be excluded without changing query collection.
                        let internal = format!("__requested_{i}");
                        let mut normalized_range = range.clone();
                        normalized_range["key"] = json!(internal);
                        normalized.push(normalized_range);
                        projection.push((internal, key));
                    }
                    intervals.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap());
                    if intervals.windows(2).any(|w| w[0].1 > w[1].0) {
                        return Err("overlapping ranges are unsupported by Tantivy 0.26.2".into());
                    }
                    range_projections.insert(name.clone(), projection);
                    *agg = json!({"range":{"field":field,"ranges":normalized}});
                }
                _ => unreachable!(),
            }
        }
        cstring(&r.query)?;
        let defaults = self
            .config
            .fields
            .iter()
            .filter(|f| f.flags & QUERY != 0)
            .map(|f| {
                self.index
                    .schema()
                    .get_field(&f.name)
                    .map_err(|e| e.to_string())
            })
            .collect::<Result<Vec<_>>>()?;
        let mut parser = QueryParser::for_index(&self.index, defaults);
        parser.set_conjunction_by_default();
        let query = parser.parse_query(&r.query).map_err(|e| e.to_string())?;
        let aggs: Aggregations =
            serde_json::from_value(Json::Object(r.aggregations)).map_err(|e| e.to_string())?;
        let limits = AggregationLimitsGuard::new(Some(r.memory_limit_bytes), Some(r.bucket_limit));
        let collector = AggregationCollector::from_aggs(
            aggs,
            AggContextParams::new(limits, self.index.tokenizers().clone()),
        );
        let result = self
            .reader
            .searcher()
            .search(&query, &collector)
            .map_err(|e| e.to_string())?;
        let mut result = serde_json::to_value(result).map_err(|e| e.to_string())?;
        for name in stats_names {
            let stats = &result[&name];
            if stats["sum"].as_f64().is_none()
                || (stats["count"].as_u64().unwrap_or(0) > 0
                    && ["min", "max", "avg"]
                        .iter()
                        .any(|k| stats[k].as_f64().is_none()))
            {
                return Err("numeric stats overflow/non-finite result".into());
            }
        }
        for (name, projection) in range_projections {
            let buckets = result[&name]["buckets"]
                .as_array()
                .ok_or("unexpected native range response")?;
            let mut selected = Vec::new();
            for (internal, key) in projection {
                let mut bucket = buckets
                    .iter()
                    .find(|b| b["key"] == internal)
                    .ok_or("missing native requested range bucket")?
                    .clone();
                bucket["key"] = json!(key);
                selected.push(bucket);
            }
            result[&name]["buckets"] = json!(selected);
        }
        serde_json::to_string(&json!({"version":3,"aggregations":result}))
            .map_err(|e| e.to_string())
    }
}
/// # Safety
/// Returned UTF-8 JSON and errors use free_string. Same handle contract as v2.
#[no_mangle]
pub unsafe extern "C" fn tantivy_ohos_aggregate_v3(
    h: *mut SearchHandle,
    request_json: *const c_char,
    out: *mut *mut c_char,
    error: *mut *mut c_char,
) -> i32 {
    if !out.is_null() {
        *out = ptr::null_mut();
    }
    boundary(error, || {
        if out.is_null() {
            return Err("null aggregation output".into());
        }
        let r = decode(string(request_json)?, 65_536)?;
        *out = cstring(&handle(h)?.aggregate(r)?)?.into_raw();
        Ok(())
    })
}
