//! Frozen v2 C layouts. New incompatible layouts require new symbol names.
use super::*;
use config::*;
use std::collections::HashSet;

#[repr(C)]
pub struct FieldSpecV2 {
    pub name: *const c_char,
    pub kind: u32,
    pub flags: u32,
}
#[repr(C)]
pub struct CreateOptionsV2 {
    pub struct_size: usize,
    pub version: u32,
    pub fields: *const FieldSpecV2,
    pub field_count: usize,
}
#[repr(C)]
pub struct OpenOptionsV2 {
    pub struct_size: usize,
    pub version: u32,
    pub mode: u32,
}
#[repr(C)]
pub struct SearchOptionsV2 {
    pub struct_size: usize,
    pub version: u32,
    pub projection: u32,
    pub fields: *const *const c_char,
    pub field_count: usize,
    pub query_fields: *const *const c_char,
    pub query_field_count: usize,
}
#[repr(C)]
pub struct FieldValueV2 {
    pub name: *const c_char,
    pub kind: u32,
    pub string_value: *const c_char,
    pub i64_value: i64,
    pub f64_value: f64,
}
#[repr(C)]
#[derive(Default)]
pub struct HitViewV2 {
    pub id: *const c_char,
    pub score: f32,
    pub field_count: usize,
}
#[repr(C)]
#[derive(Default)]
pub struct ValueViewV2 {
    pub name: *const c_char,
    pub kind: u32,
    pub string_value: *const c_char,
    pub i64_value: i64,
    pub f64_value: f64,
}

pub(super) enum StoredValue {
    String(CString),
    I64(i64),
    F64(f64),
}
pub(super) struct ResultValue {
    pub(super) name: CString,
    pub(super) kind: u32,
    pub(super) value: StoredValue,
}
pub(super) struct ResultHit {
    pub(super) id: CString,
    pub(super) score: f32,
    pub(super) values: Vec<ResultValue>,
    pub(super) sort: Option<(CString, u32, Option<StoredValue>)>,
}
pub struct SearchResult {
    pub(super) hits: Vec<ResultHit>,
    pub(super) total: Option<u64>,
}

fn version<T>(size: usize, version: u32) -> Result<()> {
    if version != 2 || size != std::mem::size_of::<T>() {
        return Err("unsupported options version or struct_size (expected v2 exact layout)".into());
    }
    Ok(())
}

// A caller must provide the complete v2 options object before this is called.
unsafe fn required<'a, T>(p: *const T, label: &str) -> Result<&'a T> {
    p.as_ref().ok_or_else(|| format!("null {label}"))
}
unsafe fn array<'a, T>(p: *const T, count: usize, max: usize) -> Result<&'a [T]> {
    if count > max {
        return Err(format!("array count exceeds {max}"));
    }
    if count == 0 {
        if !p.is_null() {
            return Err("empty arrays must use NULL/0".into());
        }
        return Ok(&[]);
    }
    if p.is_null() {
        return Err("null array with nonzero count".into());
    }
    Ok(std::slice::from_raw_parts(p, count))
}
unsafe fn output<T>(p: *mut *mut T) {
    if !p.is_null() {
        *p = ptr::null_mut();
    }
}

/// # Safety
/// All non-null pointers must be valid, aligned, and obey the public header contract.
#[no_mangle]
pub unsafe extern "C" fn tantivy_ohos_create_v2(
    path: *const c_char,
    options: *const CreateOptionsV2,
    out: *mut *mut SearchHandle,
    error: *mut *mut c_char,
) -> i32 {
    output(out);
    boundary(error, || {
        if out.is_null() {
            return Err("null output handle".into());
        }
        let o = required(options, "create options")?;
        version::<CreateOptionsV2>(o.struct_size, o.version)?;
        let mut fields = Vec::new();
        for f in array(o.fields, o.field_count, 64)? {
            fields.push(FieldConfig {
                name: string(f.name)?.into(),
                kind: f.kind,
                flags: f.flags,
            });
        }
        let h = SearchHandle::open_config(string(path)?, Some(Config::new(fields)), false)?;
        *out = Box::into_raw(Box::new(h));
        Ok(())
    })
}

/// # Safety
/// See the public header contract for all v2 functions.
#[no_mangle]
pub unsafe extern "C" fn tantivy_ohos_open_v2(
    path: *const c_char,
    options: *const OpenOptionsV2,
    out: *mut *mut SearchHandle,
    error: *mut *mut c_char,
) -> i32 {
    output(out);
    boundary(error, || {
        if out.is_null() {
            return Err("null output handle".into());
        }
        let o = required(options, "open options")?;
        version::<OpenOptionsV2>(o.struct_size, o.version)?;
        if !matches!(o.mode, 1 | 2) {
            return Err("unknown open mode".into());
        }
        *out = Box::into_raw(Box::new(SearchHandle::open_config(
            string(path)?,
            None,
            o.mode == 2,
        )?));
        Ok(())
    })
}

/// # Safety
/// See the public header contract for all v2 functions.
#[no_mangle]
pub unsafe extern "C" fn tantivy_ohos_upsert_v2(
    h: *mut SearchHandle,
    id: *const c_char,
    values: *const FieldValueV2,
    count: usize,
    error: *mut *mut c_char,
) -> i32 {
    boundary(error, || {
        let h = handle(h)?;
        h.writer()?;
        let id = string(id)?;
        if id.is_empty() {
            return Err("document id must not be empty".into());
        }
        let mut doc = TantivyDocument::default();
        doc.add_text(h.id, id);
        let mut names = HashSet::new();
        let schema = h.index.schema();
        // Validate and build the entire document before submitting any mutation.
        for v in array(values, count, 64)? {
            let name = string(v.name)?;
            if !names.insert(name) {
                return Err(format!("duplicate document field: {name}"));
            }
            let spec = h
                .config
                .fields
                .iter()
                .find(|f| f.name == name)
                .ok_or_else(|| format!("unknown document field: {name}"))?;
            if spec.kind != v.kind {
                return Err(format!("type mismatch: {name}"));
            }
            let field = schema.get_field(name).map_err(|e| e.to_string())?;
            match v.kind {
                TEXT | KEYWORD => doc.add_text(field, string(v.string_value)?),
                I64 => doc.add_i64(field, v.i64_value),
                F64 if v.f64_value.is_finite() => doc.add_f64(field, v.f64_value),
                _ => return Err("floating-point values must be finite".into()),
            }
        }
        if h.config == Config::legacy() && !names.contains("text") {
            return Err("original id/text configuration requires a text value".into());
        }
        h.replace(id, doc)
    })
}

/// # Safety
/// See the public header contract for all v2 functions.
#[no_mangle]
pub unsafe extern "C" fn tantivy_ohos_reload(h: *mut SearchHandle, error: *mut *mut c_char) -> i32 {
    boundary(error, || {
        handle(h)?.reader.reload().map_err(|e| e.to_string())
    })
}

/// # Safety
/// See the public header contract for all v2 functions.
#[no_mangle]
pub unsafe extern "C" fn tantivy_ohos_config_json_v2(
    h: *mut SearchHandle,
    out: *mut *mut c_char,
    error: *mut *mut c_char,
) -> i32 {
    output(out);
    boundary(error, || {
        if out.is_null() {
            return Err("null config output".into());
        }
        let json = serde_json::to_string(&handle(h)?.config).map_err(|e| e.to_string())?;
        *out = CString::new(json).map_err(|e| e.to_string())?.into_raw();
        Ok(())
    })
}

impl SearchHandle {
    unsafe fn search_v2(
        &self,
        query: &str,
        limit: usize,
        o: &SearchOptionsV2,
    ) -> Result<SearchResult> {
        version::<SearchOptionsV2>(o.struct_size, o.version)?;
        if limit == 0 || limit > 1000 {
            return Err("limit must be between 1 and 1000".into());
        }
        let (query, projected) = self.prepare_search(query, o)?;
        let searcher = self.reader.searcher();
        let docs = searcher
            .search(&query, &TopDocs::with_limit(limit).order_by_score())
            .map_err(|e| e.to_string())?;
        self.materialize(&searcher, docs, projected)
    }
    pub(super) unsafe fn prepare_search<'a>(
        &'a self,
        query: &str,
        o: &SearchOptionsV2,
    ) -> Result<(Box<dyn tantivy::query::Query>, Vec<&'a FieldConfig>)> {
        let schema = self.index.schema();
        let explicit = array(o.fields, o.field_count, 64)?;
        let projected: Vec<&FieldConfig> = match o.projection {
            0 | 1 if !explicit.is_empty() => {
                return Err("default/none projection requires NULL/0 fields".into())
            }
            0 => self
                .config
                .fields
                .iter()
                .filter(|f| f.flags & RETURN != 0)
                .collect(),
            1 => Vec::new(),
            2 => {
                if explicit.is_empty() {
                    return Err(
                        "explicit projection needs at least one field; use NONE for ID/score"
                            .into(),
                    );
                }
                let mut fields = Vec::new();
                let mut names = HashSet::new();
                for p in explicit {
                    let name = string(*p)?;
                    if !names.insert(name) {
                        return Err("duplicate projection field".into());
                    }
                    let f = self
                        .config
                        .fields
                        .iter()
                        .find(|f| f.name == name)
                        .ok_or_else(|| format!("unknown projection field: {name}"))?;
                    if f.flags & STORED_FLAG == 0 {
                        return Err(format!("projection field is not stored: {name}"));
                    }
                    fields.push(f);
                }
                fields
            }
            _ => return Err("unknown projection mode".into()),
        };
        let query_fields = array(o.query_fields, o.query_field_count, 65)?;
        let defaults: Vec<Field> = if query_fields.is_empty() {
            self.config
                .fields
                .iter()
                .filter(|f| f.flags & QUERY != 0)
                .map(|f| schema.get_field(&f.name).map_err(|e| e.to_string()))
                .collect::<Result<_>>()?
        } else {
            let mut fields = Vec::new();
            let mut names = HashSet::new();
            for p in query_fields {
                let name = string(*p)?;
                if !names.insert(name) {
                    return Err("duplicate query field".into());
                }
                let field = schema.get_field(name).map_err(|e| e.to_string())?;
                if name != "id"
                    && !self.config.fields.iter().any(|f| {
                        f.name == name && f.flags & INDEXED != 0 && matches!(f.kind, TEXT | KEYWORD)
                    })
                {
                    return Err(format!(
                        "query defaults require an indexed text/keyword field: {name}"
                    ));
                }
                fields.push(field);
            }
            fields
        };
        let mut parser = QueryParser::for_index(&self.index, defaults);
        parser.set_conjunction_by_default();
        let query = parser.parse_query(query).map_err(|e| e.to_string())?;
        Ok((query, projected))
    }
    pub(super) fn materialize(
        &self,
        searcher: &tantivy::Searcher,
        docs: Vec<(f32, tantivy::DocAddress)>,
        projected: Vec<&FieldConfig>,
    ) -> Result<SearchResult> {
        let schema = self.index.schema();
        let mut hits = Vec::with_capacity(docs.len());
        // Resolve fast columns once per segment, only when no stored values are needed.
        let fast_ids =
            if !docs.is_empty() && projected.is_empty() && self.config.fast_id == Some(true) {
                Some(
                    searcher
                        .segment_readers()
                        .iter()
                        .map(|segment| {
                            segment
                                .fast_fields()
                                .str("id")
                                .map_err(|e| e.to_string())?
                                .ok_or_else(|| "missing configured fast id column".to_owned())
                        })
                        .collect::<Result<Vec<_>>>()?,
                )
            } else {
                None
            };
        for (score, addr) in docs {
            finite_score(score)?;
            if let Some(columns) = &fast_ids {
                let column = &columns[addr.segment_ord as usize];
                let ord = column
                    .term_ords(addr.doc_id)
                    .next()
                    .ok_or("missing fast id")?;
                let mut id = String::new();
                if !column.ord_to_str(ord, &mut id).map_err(|e| e.to_string())? {
                    return Err("invalid fast id ordinal".into());
                }
                hits.push(ResultHit {
                    id: CString::new(id).map_err(|e| e.to_string())?,
                    score,
                    values: Vec::new(),
                    sort: None,
                });
                continue;
            }
            #[cfg(test)]
            self.docstore_reads.set(self.docstore_reads.get() + 1);
            let doc: TantivyDocument = searcher.doc(addr).map_err(|e| e.to_string())?;
            let id = doc
                .get_first(self.id)
                .and_then(|v| v.as_str())
                .ok_or("missing stored id")?;
            let mut values = Vec::with_capacity(projected.len());
            for f in &projected {
                let field = schema.get_field(&f.name).map_err(|e| e.to_string())?;
                if let Some(v) = doc.get_first(field) {
                    let value = match f.kind {
                        TEXT | KEYWORD => StoredValue::String(
                            CString::new(v.as_str().ok_or("invalid stored string")?)
                                .map_err(|e| e.to_string())?,
                        ),
                        I64 => StoredValue::I64(v.as_i64().ok_or("invalid stored i64")?),
                        F64 => StoredValue::F64(v.as_f64().ok_or("invalid stored f64")?),
                        _ => return Err("unknown stored type".into()),
                    };
                    values.push(ResultValue {
                        name: CString::new(f.name.as_str()).map_err(|e| e.to_string())?,
                        kind: f.kind,
                        value,
                    });
                }
            }
            hits.push(ResultHit {
                id: CString::new(id).map_err(|e| e.to_string())?,
                score,
                values,
                sort: None,
            });
        }
        Ok(SearchResult { hits, total: None })
    }
}

/// # Safety
/// See the public header contract for all v2 functions.
#[no_mangle]
pub unsafe extern "C" fn tantivy_ohos_search_v2(
    h: *mut SearchHandle,
    query: *const c_char,
    limit: usize,
    options: *const SearchOptionsV2,
    out: *mut *mut SearchResult,
    error: *mut *mut c_char,
) -> i32 {
    output(out);
    boundary(error, || {
        if out.is_null() {
            return Err("null result output".into());
        }
        let result =
            handle(h)?.search_v2(string(query)?, limit, required(options, "search options")?)?;
        *out = Box::into_raw(Box::new(result));
        Ok(())
    })
}

/// # Safety
/// See the public header contract for all v2 functions.
#[no_mangle]
pub unsafe extern "C" fn tantivy_ohos_result_count_v2(
    result: *const SearchResult,
    out: *mut usize,
    error: *mut *mut c_char,
) -> i32 {
    if !out.is_null() {
        *out = 0;
    }
    boundary(error, || {
        if out.is_null() {
            return Err("null count output".into());
        }
        *out = required(result, "result")?.hits.len();
        Ok(())
    })
}

/// # Safety
/// See the public header contract for all v2 functions.
#[no_mangle]
pub unsafe extern "C" fn tantivy_ohos_result_hit_v2(
    result: *const SearchResult,
    index: usize,
    out: *mut HitViewV2,
    error: *mut *mut c_char,
) -> i32 {
    if !out.is_null() {
        *out = HitViewV2::default();
    }
    boundary(error, || {
        if out.is_null() {
            return Err("null hit output".into());
        }
        let h = required(result, "result")?
            .hits
            .get(index)
            .ok_or("hit index out of range")?;
        *out = HitViewV2 {
            id: h.id.as_ptr(),
            score: h.score,
            field_count: h.values.len(),
        };
        Ok(())
    })
}

/// # Safety
/// See the public header contract for all v2 functions.
#[no_mangle]
pub unsafe extern "C" fn tantivy_ohos_result_value_v2(
    result: *const SearchResult,
    hit: usize,
    field: usize,
    out: *mut ValueViewV2,
    error: *mut *mut c_char,
) -> i32 {
    if !out.is_null() {
        *out = ValueViewV2::default();
    }
    boundary(error, || {
        if out.is_null() {
            return Err("null value output".into());
        }
        let h = required(result, "result")?
            .hits
            .get(hit)
            .ok_or("hit index out of range")?;
        let v = h.values.get(field).ok_or("value index out of range")?;
        let mut view = ValueViewV2 {
            name: v.name.as_ptr(),
            kind: v.kind,
            ..Default::default()
        };
        match &v.value {
            StoredValue::String(s) => view.string_value = s.as_ptr(),
            StoredValue::I64(n) => view.i64_value = *n,
            StoredValue::F64(n) => view.f64_value = *n,
        }
        *out = view;
        Ok(())
    })
}

/// # Safety
/// result must be NULL or a live result from search_v2, freed exactly once.
#[no_mangle]
pub unsafe extern "C" fn tantivy_ohos_free_result_v2(result: *mut SearchResult) {
    let _ = catch_unwind(AssertUnwindSafe(|| {
        if !result.is_null() {
            drop(Box::from_raw(result));
        }
    }));
}
