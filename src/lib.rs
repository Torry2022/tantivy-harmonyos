mod advanced;
mod api;
pub use advanced::*;
mod config;
mod readonly;
#[cfg(test)]
mod tests;
#[cfg(test)]
mod tests_v2;
#[cfg(test)]
mod tests_v3;
#[cfg(test)]
mod tests_v4;
mod tokenizer;
pub use api::*;

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
use tantivy::{Index, IndexReader, IndexWriter, ReloadPolicy, Term};

type Result<T> = std::result::Result<T, String>;

fn finite_score(score: f32) -> Result<()> {
    if !score.is_finite() {
        return Err("non-finite search score: query boost or scoring arithmetic overflow".into());
    }
    Ok(())
}

pub struct SearchHandle {
    index: Index,
    reader: IndexReader,
    writer: Option<IndexWriter>,
    config: config::Config,
    id: Field,
    text: Field,
    #[cfg(test)]
    docstore_reads: std::cell::Cell<usize>,
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
        let h = Self::open_config(path, create.then(config::Config::legacy), false)?;
        h.require_legacy()?;
        Ok(h)
    }
    fn open_config(path: &str, create: Option<config::Config>, read_only: bool) -> Result<Self> {
        let (index, config) = if let Some(config) = create {
            let expected = config.schema()?;
            // Create only in a new directory; never overwrite another index.
            std::fs::create_dir(path).map_err(|e| e.to_string())?;
            let index = Index::create_in_dir(path, expected).map_err(|e| e.to_string())?;
            config.save(path)?;
            (index, config)
        } else {
            let index = if read_only {
                Index::open(readonly::ReadOnlyDirectory::open(path)?)
            } else {
                Index::open_in_dir(Path::new(path))
            }
            .map_err(|e| e.to_string())?;
            let config = config::Config::load(path, &index.schema())?;
            (index, config)
        };
        if config.version == 3 {
            let jieba = tokenizer::configured_jieba(config.dictionary.as_ref().unwrap());
            index.tokenizers().register(
                &config.tokenizer,
                TextAnalyzer::builder(tokenizer::ConfiguredTokenizer(jieba))
                    .filter(LowerCaser)
                    .build(),
            );
        } else {
            index.tokenizers().register(
                "zh_jieba_v1",
                TextAnalyzer::builder(tokenizer::ChineseTokenizer)
                    .filter(LowerCaser)
                    .build(),
            );
        }
        let id = index.schema().get_field("id").map_err(|e| e.to_string())?;
        let text = index.schema().get_field("text").unwrap_or(id);
        let writer = if read_only {
            None
        } else {
            Some(
                index
                    .writer_with_num_threads(1, 15_000_000)
                    .map_err(|e| e.to_string())?,
            )
        };
        let reader = index
            .reader_builder()
            .reload_policy(ReloadPolicy::Manual)
            .try_into()
            .map_err(|e: tantivy::TantivyError| e.to_string())?;
        Ok(Self {
            index,
            reader,
            writer,
            config,
            id,
            text,
            #[cfg(test)]
            docstore_reads: std::cell::Cell::new(0),
        })
    }
    fn upsert(&mut self, id: &str, text: &str) -> Result<()> {
        self.require_legacy()?;
        if id.is_empty() {
            return Err("document id must not be empty".into());
        }
        let mut doc = TantivyDocument::default();
        doc.add_text(self.id, id);
        doc.add_text(self.text, text);
        self.replace(id, doc)
    }
    fn require_legacy(&self) -> Result<()> {
        if self.config != config::Config::legacy() {
            return Err(
                "legacy text API requires the original id/text configuration; use v2".into(),
            );
        }
        Ok(())
    }
    fn writer(&mut self) -> Result<&mut IndexWriter> {
        self.writer
            .as_mut()
            .ok_or("read-only handle: write operation is not allowed".into())
    }
    fn replace(&mut self, id: &str, document: TantivyDocument) -> Result<()> {
        // Preserve delete-before-add ordering, including multiple replacements
        // of the same ID before a commit. Callers validate before this batch.
        let term = Term::from_field_text(self.id, id);
        self.writer()?
            .run([
                tantivy::indexer::UserOperation::Delete(term),
                tantivy::indexer::UserOperation::Add(document),
            ])
            .map_err(|e| e.to_string())?;
        Ok(())
    }
    fn delete(&mut self, id: &str) -> Result<()> {
        if id.is_empty() {
            return Err("document id must not be empty".into());
        }
        let term = Term::from_field_text(self.id, id);
        self.writer()?.delete_term(term);
        Ok(())
    }
    fn commit(&mut self) -> Result<()> {
        self.writer()?.commit().map_err(|e| e.to_string())?;
        self.reader.reload().map_err(|e| e.to_string())
    }
    fn search(&self, query: &str, limit: usize) -> Result<String> {
        self.require_legacy()?;
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
            finite_score(score)?;
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
    4
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
    boundary(error, || handle(h)?.delete(string(id)?))
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
    let _ = catch_unwind(AssertUnwindSafe(|| {
        if !h.is_null() {
            drop(Box::from_raw(h));
        }
    }));
}

/// # Safety
/// value must be null or an unfreed string returned by this library.
#[no_mangle]
pub unsafe extern "C" fn tantivy_ohos_free_string(value: *mut c_char) {
    let _ = catch_unwind(AssertUnwindSafe(|| {
        if !value.is_null() {
            drop(CString::from_raw(value));
        }
    }));
}
