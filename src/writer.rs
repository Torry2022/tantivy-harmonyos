//! Per-handle writer settings; never saved as index metadata.
use super::*;
use std::mem::size_of;

#[repr(C)]
pub struct WriterOptions {
    pub struct_size: usize,
    pub num_threads: u32,
    pub memory_budget_bytes: u64,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub(super) struct WriterConfig {
    pub num_threads: usize,
    pub memory_budget_bytes: usize,
}

impl Default for WriterConfig {
    fn default() -> Self {
        Self {
            num_threads: 1,
            memory_budget_bytes: 15_000_000,
        }
    }
}

impl WriterConfig {
    fn validate(self) -> Result<Self> {
        // Match the pinned Tantivy 0.26.2 arena bounds. Its automatic writer
        // selection caps workers at 8; the SDK applies that bound explicitly.
        if !(1..=8).contains(&self.num_threads) {
            return Err("writer num_threads must be between 1 and 8".into());
        }
        let per_thread = self.memory_budget_bytes / self.num_threads;
        if !(15_000_000..4_293_967_295).contains(&per_thread) {
            return Err("writer memory budget per thread must be at least 15000000 and less than 4293967295 bytes".into());
        }
        Ok(self)
    }

    unsafe fn from_options(options: *const WriterOptions) -> Result<Self> {
        let Some(options) = options.as_ref() else {
            return Ok(Self::default());
        };
        if options.struct_size != size_of::<WriterOptions>() {
            return Err("unsupported writer options struct_size".into());
        }
        Self {
            num_threads: options.num_threads as usize,
            memory_budget_bytes: usize::try_from(options.memory_budget_bytes)
                .map_err(|_| "writer memory budget exceeds platform size")?,
        }
        .validate()
    }
}

/// # Safety
/// See the public C header's pointer, ownership and serialization rules.
#[no_mangle]
pub unsafe extern "C" fn tantivy_ohos_create(
    path: *const c_char,
    config_json: *const c_char,
    writer_options: *const WriterOptions,
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
        let settings = WriterConfig::from_options(writer_options)?;
        let config = advanced::creation_config(string(config_json)?)?;
        *out = Box::into_raw(Box::new(SearchHandle::open_config_with_writer(
            string(path)?,
            Some(config),
            Some(settings),
        )?));
        Ok(())
    })
}

/// # Safety
/// See the public C header's pointer, ownership and serialization rules.
#[no_mangle]
pub unsafe extern "C" fn tantivy_ohos_open(
    path: *const c_char,
    options: *const OpenOptions,
    writer_options: *const WriterOptions,
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
        let options = options.as_ref().ok_or("null open options")?;
        if options.struct_size != size_of::<OpenOptions>() {
            return Err("unsupported open options struct_size".into());
        }
        let settings = match options.mode {
            1 => Some(WriterConfig::from_options(writer_options)?),
            2 if writer_options.is_null() => None,
            2 => return Err("read-only open requires NULL writer options".into()),
            _ => return Err("unknown open mode".into()),
        };
        *out = Box::into_raw(Box::new(SearchHandle::open_config_with_writer(
            string(path)?,
            None,
            settings,
        )?));
        Ok(())
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn writer_budget_bounds_and_default() {
        assert_eq!(
            WriterConfig::default(),
            WriterConfig {
                num_threads: 1,
                memory_budget_bytes: 15_000_000
            }
        );
        for (threads, budget, valid) in [
            (0, 15_000_000, false),
            (9, 135_000_000, false),
            (1, 14_999_999, false),
            (1, 15_000_000, true),
            (2, 29_999_999, false),
            (2, 30_000_001, true),
            (8, 120_000_000, true),
            (1, 4_293_967_294, true),
            (1, 4_293_967_295, false),
            (8, usize::MAX, false),
        ] {
            assert_eq!(
                WriterConfig {
                    num_threads: threads,
                    memory_budget_bytes: budget
                }
                .validate()
                .is_ok(),
                valid
            );
        }
    }
}
