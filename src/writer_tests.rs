use super::*;
use std::mem::{offset_of, size_of};

const CONFIG: &str =
    r#"{"tokenizer":"zh_jieba_v3","fields":[{"name":"title","kind":1,"flags":15}]}"#;
fn c(s: &str) -> CString {
    CString::new(s).unwrap()
}
unsafe fn call(f: impl FnOnce(*mut *mut c_char) -> i32) -> Result<()> {
    let mut error = ptr::null_mut();
    let status = f(&mut error);
    let message = if error.is_null() {
        String::new()
    } else {
        CStr::from_ptr(error).to_str().unwrap().into()
    };
    tantivy_ohos_free_string(error);
    if status == 0 {
        assert!(message.is_empty());
        Ok(())
    } else {
        assert_eq!(status, 1);
        assert!(!message.is_empty());
        Err(message)
    }
}
fn options(threads: u32, bytes: u64) -> WriterOptions {
    WriterOptions {
        struct_size: size_of::<WriterOptions>(),

        num_threads: threads,
        memory_budget_bytes: bytes,
    }
}

#[test]
fn writer_layout_and_invalid_create_have_no_filesystem_effects() {
    assert_eq!(size_of::<WriterOptions>(), 24);
    assert_eq!(offset_of!(WriterOptions, struct_size), 0);
    assert_eq!(offset_of!(WriterOptions, num_threads), 8);
    assert_eq!(offset_of!(WriterOptions, memory_budget_bytes), 16);
    unsafe {
        let path = tests::fixture_path("v5-invalid");
        let mut wrong_size = options(1, 15_000_000);
        wrong_size.struct_size -= 1;
        for invalid in [
            options(0, 15_000_000),
            options(9, 135_000_000),
            options(2, 15_000_000),
            options(1, u64::MAX),
            wrong_size,
        ] {
            let mut h = ptr::dangling_mut();
            assert!(call(|e| tantivy_ohos_create(
                c(&path).as_ptr(),
                c(CONFIG).as_ptr(),
                &invalid,
                &mut h,
                e
            ))
            .is_err());
            assert!(h.is_null());
            assert!(!Path::new(&path).exists());
        }
        assert!(call(|e| tantivy_ohos_create(
            c(&path).as_ptr(),
            c(CONFIG).as_ptr(),
            ptr::null(),
            ptr::null_mut(),
            e
        ))
        .is_err());
        assert!(!Path::new(&path).exists());
    }
}

#[test]
fn custom_writer_reopen_and_reader_preserve_index_configuration() {
    unsafe {
        let path = tests::fixture_path("v5-lifecycle");
        let mut h = ptr::null_mut();
        call(|e| {
            tantivy_ohos_create(
                c(&path).as_ptr(),
                c(CONFIG).as_ptr(),
                &options(2, 30_000_000),
                &mut h,
                e,
            )
        })
        .unwrap();
        let title = c("title");
        let text = c("鸿蒙检索 alpha");
        let value = FieldValue {
            name: title.as_ptr(),
            kind: 1,
            string_value: text.as_ptr(),
            i64_value: 0,
            f64_value: 0.0,
        };
        call(|e| tantivy_ohos_upsert(h, c("a").as_ptr(), &value, 1, e)).unwrap();
        call(|e| tantivy_ohos_commit(h, e)).unwrap();
        tantivy_ohos_close(h);
        let metadata = std::fs::read(Path::new(&path).join("tantivy-ohos.json")).unwrap();
        let rw = OpenOptions {
            struct_size: size_of::<OpenOptions>(),

            mode: 1,
        };
        call(|e| tantivy_ohos_open(c(&path).as_ptr(), &rw, &options(1, 16_000_000), &mut h, e))
            .unwrap();
        call(|e| tantivy_ohos_upsert(h, c("b").as_ptr(), &value, 1, e)).unwrap();
        call(|e| tantivy_ohos_commit(h, e)).unwrap();
        tantivy_ohos_close(h);
        assert_eq!(
            std::fs::read(Path::new(&path).join("tantivy-ohos.json")).unwrap(),
            metadata
        );
        let ro = OpenOptions { mode: 2, ..rw };
        // Reader access uses the same index independently of runtime writer settings.
        call(|e| tantivy_ohos_open(c(&path).as_ptr(), &ro, ptr::null(), &mut h, e)).unwrap();
        let mut result = ptr::null_mut();
        call(|e| {
            tantivy_ohos_search(
                h,
                c(r#"{"query":"alpha","limit":10,"projection":"none","track_total":true}"#)
                    .as_ptr(),
                &mut result,
                e,
            )
        })
        .unwrap();
        let mut total = 0;
        let mut exact = false;
        call(|e| tantivy_ohos_result_total(result, &mut total, &mut exact, e)).unwrap();
        assert_eq!((total, exact), (2, true));
        tantivy_ohos_free_result(result);
        tantivy_ohos_close(h);
        h = ptr::dangling_mut();
        assert!(call(|e| tantivy_ohos_open(
            c(&path).as_ptr(),
            &ro,
            &options(1, 15_000_000),
            &mut h,
            e
        ))
        .unwrap_err()
        .contains("NULL writer options"));
        assert!(h.is_null());
        call(|e| tantivy_ohos_open(c(&path).as_ptr(), &ro, ptr::null(), &mut h, e)).unwrap();
        assert!((*h).writer.is_none());
        tantivy_ohos_close(h);
        // Invalid runtime options do not modify an existing index's configuration.
        assert!(
            call(|e| tantivy_ohos_open(c(&path).as_ptr(), &rw, &options(2, 1), &mut h, e)).is_err()
        );
        assert!(h.is_null());
        assert_eq!(
            std::fs::read(Path::new(&path).join("tantivy-ohos.json")).unwrap(),
            metadata
        );
        std::fs::remove_dir_all(path).unwrap();
    }
}

#[test]
fn null_writer_options_keep_defaults_on_reopen() {
    unsafe {
        let path = tests::fixture_path("writer-defaults");
        let mut h = ptr::null_mut();
        call(|e| {
            tantivy_ohos_create(
                c(&path).as_ptr(),
                c(CONFIG).as_ptr(),
                ptr::null(),
                &mut h,
                e,
            )
        })
        .unwrap();
        assert!((*h).writer.is_some());
        tantivy_ohos_close(h);
        let rw = OpenOptions {
            struct_size: size_of::<OpenOptions>(),
            mode: 1,
        };
        call(|e| tantivy_ohos_open(c(&path).as_ptr(), &rw, ptr::null(), &mut h, e)).unwrap();
        assert!((*h).writer.is_some());
        tantivy_ohos_close(h);
        std::fs::remove_dir_all(path).unwrap();
    }
}
