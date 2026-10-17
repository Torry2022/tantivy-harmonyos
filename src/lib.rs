#[cfg(test)]
mod tests;
mod tokenizer;

use serde::Serialize;
use std::ffi::{c_char, CStr, CString};
use std::panic::{catch_unwind, AssertUnwindSafe};
use std::path::Path;
use std::ptr;
use tantivy::collector::TopDocs;
use tantivy::query::QueryParser;
use tantivy::schema::{
    Field, IndexRecordOption, Schema, TantivyDocument, TextFieldIndexing, TextOptions, Value,
    STORED, STRING,
};
use tantivy::tokenizer::{LowerCaser, TextAnalyzer};
use tantivy::{doc, Index, IndexReader, IndexWriter, ReloadPolicy, Term};

type Result<T> = std::result::Result<T, String>;

pub struct SearchHandle {
    index: Index,
    reader: IndexReader,
    writer: IndexWriter,
    id: Field,
    text: Field,
}

#[derive(Serialize)]
struct Hit {
    id: String,
    text: String,
    score: f32,
}

fn schema() -> Schema {
    let mut builder = Schema::builder();
    builder.add_text_field("id", STRING | STORED);
    builder.add_text_field(
        "text",
        TextOptions::default().set_stored().set_indexing_options(
            TextFieldIndexing::default()
                .set_tokenizer("zh_jieba_v1")
                .set_index_option(IndexRecordOption::WithFreqsAndPositions),
        ),
    );
    builder.build()
}

impl SearchHandle {
    fn open(path: &str, create: bool) -> Result<Self> {
        let expected = schema();
        let index = if create {
            // Create only in a new directory; never overwrite another index.
            std::fs::create_dir(path).map_err(|e| e.to_string())?;
            Index::create_in_dir(path, expected.clone()).map_err(|e| e.to_string())?
        } else {
            Index::open_in_dir(Path::new(path)).map_err(|e| e.to_string())?
        };
        if index.schema() != expected {
            return Err("unsupported index schema".into());
        }
        index.tokenizers().register(
            "zh_jieba_v1",
            TextAnalyzer::builder(tokenizer::ChineseTokenizer)
                .filter(LowerCaser)
                .build(),
        );
        let id = expected.get_field("id").map_err(|e| e.to_string())?;
        let text = expected.get_field("text").map_err(|e| e.to_string())?;
        let writer = index
            .writer_with_num_threads(1, 15_000_000)
            .map_err(|e| e.to_string())?;
        let reader = index
            .reader_builder()
            .reload_policy(ReloadPolicy::Manual)
            .try_into()
            .map_err(|e: tantivy::TantivyError| e.to_string())?;
        Ok(Self {
            index,
            reader,
            writer,
            id,
            text,
        })
    }
    fn upsert(&mut self, id: &str, text: &str) -> Result<()> {
        if id.is_empty() {
            return Err("document id must not be empty".into());
        }
        self.writer.delete_term(Term::from_field_text(self.id, id));
        self.writer
            .add_document(doc!(self.id => id, self.text => text))
            .map_err(|e| e.to_string())?;
        Ok(())
    }
    fn commit(&mut self) -> Result<()> {
        self.writer.commit().map_err(|e| e.to_string())?;
        self.reader.reload().map_err(|e| e.to_string())
    }
    fn search(&self, query: &str, limit: usize) -> Result<String> {
        if limit == 0 || limit > 1000 {
            return Err("limit must be between 1 and 1000".into());
        }
        let mut parser = QueryParser::for_index(&self.index, vec![self.text]);
        parser.set_conjunction_by_default();
        let query = parser.parse_query(query).map_err(|e| e.to_string())?;
        let searcher = self.reader.searcher();
        let docs = searcher
            .search(&query, &TopDocs::with_limit(limit).order_by_score())
            .map_err(|e| e.to_string())?;
        let mut hits = Vec::with_capacity(docs.len());
        for (score, address) in docs {
            let doc: TantivyDocument = searcher.doc(address).map_err(|e| e.to_string())?;
            hits.push(Hit {
                id: doc
                    .get_first(self.id)
                    .and_then(|v| v.as_str())
                    .ok_or("missing stored id")?
                    .to_owned(),
                text: doc
                    .get_first(self.text)
                    .and_then(|v| v.as_str())
                    .ok_or("missing stored text")?
                    .to_owned(),
                score,
            });
        }
        serde_json::to_string(&hits).map_err(|e| e.to_string())
    }
}

// C callers own returned strings and must release them using tantivy_ohos_free_string.
// Handles are exclusive: callers must serialize operations, including close.
unsafe fn string<'a>(value: *const c_char) -> Result<&'a str> {
    if value.is_null() {
        return Err("null string".into());
    }
    CStr::from_ptr(value).to_str().map_err(|e| e.to_string())
}

unsafe fn handle<'a>(value: *mut SearchHandle) -> Result<&'a mut SearchHandle> {
    value.as_mut().ok_or("null handle".into())
}

fn boundary(error: *mut *mut c_char, operation: impl FnOnce() -> Result<()>) -> i32 {
    if !error.is_null() {
        unsafe {
            *error = ptr::null_mut();
        }
    }
    match catch_unwind(AssertUnwindSafe(operation)) {
        Ok(Ok(())) => 0,
        result => {
            let message = match result {
                Ok(Err(e)) => e,
                _ => "Rust panic in search operation".into(),
            };
            if !error.is_null() {
                let safe = message.replace('\0', "\\0");
                unsafe {
                    *error = CString::new(safe).unwrap().into_raw();
                }
            }
            1
        }
    }
}

#[no_mangle]
pub extern "C" fn tantivy_ohos_abi_version() -> u32 {
    1
}

/// # Safety
/// Non-null pointers must be valid; out and error must point to writable pointer slots.
#[no_mangle]
pub unsafe extern "C" fn tantivy_ohos_open(
    path: *const c_char,
    create: bool,
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
        *out = Box::into_raw(Box::new(SearchHandle::open(string(path)?, create)?));
        Ok(())
    })
}

/// # Safety
/// h must be a live, exclusively accessed handle; strings and error must be valid.
#[no_mangle]
pub unsafe extern "C" fn tantivy_ohos_upsert(
    h: *mut SearchHandle,
    id: *const c_char,
    text: *const c_char,
    error: *mut *mut c_char,
) -> i32 {
    boundary(error, || handle(h)?.upsert(string(id)?, string(text)?))
}

/// # Safety
/// h must be a live, exclusively accessed handle; id and error must be valid.
#[no_mangle]
pub unsafe extern "C" fn tantivy_ohos_delete(
    h: *mut SearchHandle,
    id: *const c_char,
    error: *mut *mut c_char,
) -> i32 {
    boundary(error, || {
        let h = handle(h)?;
        let id = string(id)?;
        if id.is_empty() {
            return Err("document id must not be empty".into());
        }
        h.writer.delete_term(Term::from_field_text(h.id, id));
        Ok(())
    })
}

/// # Safety
/// h must be a live, exclusively accessed handle and error must be valid if non-null.
#[no_mangle]
pub unsafe extern "C" fn tantivy_ohos_commit(h: *mut SearchHandle, error: *mut *mut c_char) -> i32 {
    boundary(error, || handle(h)?.commit())
}

/// # Safety
/// h must be live and exclusive; query, out_json and error must be valid.
#[no_mangle]
pub unsafe extern "C" fn tantivy_ohos_search(
    h: *mut SearchHandle,
    query: *const c_char,
    limit: usize,
    out_json: *mut *mut c_char,
    error: *mut *mut c_char,
) -> i32 {
    if !out_json.is_null() {
        *out_json = ptr::null_mut();
    }
    boundary(error, || {
        if out_json.is_null() {
            return Err("null JSON output".into());
        }
        let json = handle(h)?.search(string(query)?, limit)?;
        *out_json = CString::new(json).map_err(|e| e.to_string())?.into_raw();
        Ok(())
    })
}

/// # Safety
/// h must be null or a live, exclusive handle returned by open. It cannot be reused after close.
#[no_mangle]
pub unsafe extern "C" fn tantivy_ohos_close(h: *mut SearchHandle) {
    if !h.is_null() {
        drop(Box::from_raw(h));
    }
}

/// # Safety
/// value must be null or an unfreed string returned by this library.
#[no_mangle]
pub unsafe extern "C" fn tantivy_ohos_free_string(value: *mut c_char) {
    if !value.is_null() {
        drop(CString::from_raw(value));
    }
}
