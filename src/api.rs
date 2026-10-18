//! C values, query preparation and owned search results.
use super::*;
use config::*;
use std::collections::HashSet;

#[repr(C)]
pub struct OpenOptions {
    pub struct_size: usize,
    pub mode: u32,
}
pub(super) struct QueryOptions {
    pub projection: u32,
    pub fields: *const *const c_char,
    pub field_count: usize,
    pub query_fields: *const *const c_char,
    pub query_field_count: usize,
}
#[repr(C)]
pub struct FieldValue {
    pub name: *const c_char,
    pub kind: u32,
    pub string_value: *const c_char,
    pub i64_value: i64,
    pub f64_value: f64,
}
#[repr(C)]
#[derive(Default)]
pub struct HitView {
    pub id: *const c_char,
    pub score: f32,
    pub field_count: usize,
}
#[repr(C)]
#[derive(Default)]
pub struct ValueView {
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

// Non-null pointers must satisfy the public header contract.
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
/// See the public header contract.
#[no_mangle]
pub unsafe extern "C" fn tantivy_ohos_upsert(
    h: *mut SearchHandle,
    id: *const c_char,
    values: *const FieldValue,
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
        h.replace(id, doc)
    })
}

/// # Safety
/// See the public header contract.
#[no_mangle]
pub unsafe extern "C" fn tantivy_ohos_reload(h: *mut SearchHandle, error: *mut *mut c_char) -> i32 {
    boundary(error, || {
        handle(h)?.reader.reload().map_err(|e| e.to_string())
    })
}

/// # Safety
/// See the public header contract.
#[no_mangle]
pub unsafe extern "C" fn tantivy_ohos_config_json(
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
    pub(super) unsafe fn prepare_search<'a>(
        &'a self,
        query: &str,
        o: &QueryOptions,
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
/// See the public header contract.
#[no_mangle]
pub unsafe extern "C" fn tantivy_ohos_result_count(
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
/// See the public header contract.
#[no_mangle]
pub unsafe extern "C" fn tantivy_ohos_result_hit(
    result: *const SearchResult,
    index: usize,
    out: *mut HitView,
    error: *mut *mut c_char,
) -> i32 {
    if !out.is_null() {
        *out = HitView::default();
    }
    boundary(error, || {
        if out.is_null() {
            return Err("null hit output".into());
        }
        let h = required(result, "result")?
            .hits
            .get(index)
            .ok_or("hit index out of range")?;
        *out = HitView {
            id: h.id.as_ptr(),
            score: h.score,
            field_count: h.values.len(),
        };
        Ok(())
    })
}

/// # Safety
/// See the public header contract.
#[no_mangle]
pub unsafe extern "C" fn tantivy_ohos_result_value(
    result: *const SearchResult,
    hit: usize,
    field: usize,
    out: *mut ValueView,
    error: *mut *mut c_char,
) -> i32 {
    if !out.is_null() {
        *out = ValueView::default();
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
        let mut view = ValueView {
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
/// result must be NULL or a live result from search, freed exactly once.
#[no_mangle]
pub unsafe extern "C" fn tantivy_ohos_free_result(result: *mut SearchResult) {
    let _ = catch_unwind(AssertUnwindSafe(|| {
        if !result.is_null() {
            drop(Box::from_raw(result));
        }
    }));
}
