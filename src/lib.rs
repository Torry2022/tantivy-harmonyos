mod advanced;
mod api;
pub use advanced::*;
#[cfg(test)]
mod advanced_tests;
mod config;
mod readonly;
#[cfg(test)]
mod search_tests;
#[cfg(test)]
mod tests;
mod tokenizer;
mod writer;
#[cfg(test)]
mod writer_tests;
pub use api::*;
pub use writer::*;

use std::ffi::{c_char, CStr, CString};
use std::panic::{catch_unwind, AssertUnwindSafe};
use std::path::Path;
use std::ptr;
use tantivy::collector::TopDocs;
use tantivy::query::QueryParser;
use tantivy::schema::{Field, Schema, TantivyDocument, Value};
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
    #[cfg(test)]
    docstore_reads: std::cell::Cell<usize>,
}

impl SearchHandle {
    fn open_config_with_writer(
        path: &str,
        create: Option<config::Config>,
        writer_config: Option<writer::WriterConfig>,
    ) -> Result<Self> {
        let read_only = writer_config.is_none();
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
        let writer = if let Some(settings) = writer_config {
            Some(
                index
                    .writer_with_num_threads(settings.num_threads, settings.memory_budget_bytes)
                    .map_err(|e| e.to_string())?,
            )
        } else {
            None
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
            #[cfg(test)]
            docstore_reads: std::cell::Cell::new(0),
        })
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
