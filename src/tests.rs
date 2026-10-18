use super::*;
use serde_json::{json, Value};
use std::mem::size_of;

pub(super) fn fixture_path(label: &str) -> String {
    std::env::temp_dir()
        .join(format!(
            "tantivy-ohos-{label}-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ))
        .to_str()
        .unwrap()
        .to_owned()
}
fn c(s: &str) -> CString {
    CString::new(s).unwrap()
}
unsafe fn call(f: impl FnOnce(*mut *mut c_char) -> i32) -> Result<()> {
    let mut error = ptr::null_mut();
    let code = f(&mut error);
    let message = if error.is_null() {
        String::new()
    } else {
        CStr::from_ptr(error).to_str().unwrap().into()
    };
    tantivy_ohos_free_string(error);
    if code == 0 {
        assert!(message.is_empty());
        Ok(())
    } else {
        assert_eq!(code, 1);
        assert!(!message.is_empty());
        Err(message)
    }
}
unsafe fn create(path: &str, config: Value) -> *mut SearchHandle {
    let mut h = ptr::null_mut();
    call(|e| {
        tantivy_ohos_create(
            c(path).as_ptr(),
            c(&config.to_string()).as_ptr(),
            ptr::null(),
            &mut h,
            e,
        )
    })
    .unwrap();
    h
}
unsafe fn open(path: &str, mode: u32) -> *mut SearchHandle {
    let mut h = ptr::null_mut();
    let options = OpenOptions {
        struct_size: size_of::<OpenOptions>(),
        mode,
    };
    call(|e| tantivy_ohos_open(c(path).as_ptr(), &options, ptr::null(), &mut h, e)).unwrap();
    h
}
unsafe fn put(h: *mut SearchHandle, id: &str, text: &str) {
    let text = c(text);
    let value = FieldValue {
        name: c"text".as_ptr(),
        kind: 1,
        string_value: text.as_ptr(),
        i64_value: 0,
        f64_value: 0.0,
    };
    call(|e| tantivy_ohos_upsert(h, c(id).as_ptr(), &value, 1, e)).unwrap();
}
unsafe fn query(h: *mut SearchHandle, q: &str) -> *mut SearchResult {
    let mut r = ptr::null_mut();
    call(|e| {
        tantivy_ohos_search(
            h,
            c(&json!({"query":q,"limit":20}).to_string()).as_ptr(),
            &mut r,
            e,
        )
    })
    .unwrap();
    r
}
unsafe fn ids(h: *mut SearchHandle, q: &str) -> Vec<String> {
    let r = query(h, q);
    let mut ids = (*r)
        .hits
        .iter()
        .map(|h| h.id.to_str().unwrap().into())
        .collect::<Vec<_>>();
    tantivy_ohos_free_result(r);
    ids.sort();
    ids
}
fn config() -> Value {
    json!({"fields":[{"name":"text","kind":1,"flags":15}]})
}

#[test]
fn chinese_lifecycle_snapshots_and_result_ownership() {
    unsafe {
        let path = fixture_path("lifecycle");
        let w = create(&path, config());
        let ro = open(&path, 2);
        put(w, "a", "鸿蒙设备支持中文检索 Qwen3 16GB");
        put(w, "b", "鸿蒙设备提供离线语音");
        assert!(ids(w, "鸿蒙").is_empty());
        call(|e| tantivy_ohos_commit(w, e)).unwrap();
        assert!(ids(ro, "鸿蒙").is_empty());
        call(|e| tantivy_ohos_reload(ro, e)).unwrap();
        assert_eq!(ids(ro, "鸿蒙"), ["a", "b"]);
        assert_eq!(ids(w, "鸿蒙 AND 检索"), ["a"]);
        assert_eq!(ids(w, "\"鸿蒙 设备\""), ["a", "b"]);
        assert_eq!(ids(w, "qwen3"), ["a"]);
        assert_eq!(ids(w, "16GB"), ["a"]);
        let result = query(ro, "检索");
        call(|e| tantivy_ohos_delete(w, c"b".as_ptr(), e)).unwrap();
        put(w, "a", "鸿蒙设备更新资料");
        call(|e| tantivy_ohos_commit(w, e)).unwrap();
        put(w, "uncommitted", "鸿蒙设备");
        // Stop fixture merges before comparing readonly files and timestamps.
        (*w).writer.take().unwrap().wait_merging_threads().unwrap();
        tantivy_ohos_close(w);
        tantivy_ohos_close(ro);
        assert_eq!((&(*result).hits)[0].id.to_str().unwrap(), "a");
        assert_eq!((&(*result).hits)[0].values.len(), 1);
        let mut view = HitView::default();
        call(|e| tantivy_ohos_result_hit(result, 0, &mut view, e)).unwrap();
        assert_eq!(CStr::from_ptr(view.id).to_str().unwrap(), "a");
        assert!(call(|e| tantivy_ohos_result_hit(result, 1, &mut view, e)).is_err());
        assert!(view.id.is_null());
        tantivy_ohos_free_result(result);
        let before = snapshot(&path);
        let ro = open(&path, 2);
        assert_eq!(ids(ro, "鸿蒙"), ["a"]);
        assert!(ids(ro, "检索").is_empty());
        assert!(call(|e| tantivy_ohos_commit(ro, e))
            .unwrap_err()
            .contains("read-only"));
        assert!(call(|e| tantivy_ohos_delete(ro, c"a".as_ptr(), e)).is_err());
        call(|e| tantivy_ohos_reload(ro, e)).unwrap();
        tantivy_ohos_close(ro);
        assert_eq!(before, snapshot(&path));
        std::fs::remove_dir_all(path).unwrap();
    }
}
fn snapshot(path: &str) -> std::collections::BTreeMap<String, (Vec<u8>, std::time::SystemTime)> {
    std::fs::read_dir(path)
        .unwrap()
        .map(|p| {
            let p = p.unwrap().path();
            (
                p.file_name().unwrap().to_str().unwrap().into(),
                (
                    std::fs::read(&p).unwrap(),
                    p.metadata().unwrap().modified().unwrap(),
                ),
            )
        })
        .collect()
}

#[test]
fn invalid_inputs_leave_outputs_and_documents_intact() {
    unsafe {
        let path = fixture_path("invalid");
        let h = create(&path, config());
        put(h, "a", "原有文档");
        call(|e| tantivy_ohos_commit(h, e)).unwrap();
        for (name, kind) in [("unknown", 1), ("text", 3)] {
            let name = c(name);
            let v = FieldValue {
                name: name.as_ptr(),
                kind,
                string_value: c"bad".as_ptr(),
                i64_value: 0,
                f64_value: 0.0,
            };
            assert!(call(|e| tantivy_ohos_upsert(h, c"a".as_ptr(), &v, 1, e)).is_err());
        }
        assert!(call(|e| tantivy_ohos_upsert(h, c"a".as_ptr(), ptr::null(), 1, e)).is_err());
        assert!(call(|e| tantivy_ohos_upsert(h, c"a".as_ptr(), ptr::null(), 65, e)).is_err());
        assert!(call(|e| tantivy_ohos_delete(h, c"".as_ptr(), e)).is_err());
        call(|e| tantivy_ohos_commit(h, e)).unwrap();
        assert_eq!(ids(h, "原有"), ["a"]);
        for request in [
            r#"{"query":"*","limit":0}"#,
            r#"{"query":"*","limit":1,"offset":10000}"#,
            r#"{"query":"*","limit":1,"projection":"fields","fields":["unknown"]}"#,
            r#"{"query":"*","limit":1,"projection":"none","fields":["text"]}"#,
            r#"{"query":"*","limit":1,"version":3}"#,
            r#"{"query":"\u0000","limit":1}"#,
        ] {
            let mut out = ptr::dangling_mut();
            assert!(call(|e| tantivy_ohos_search(h, c(request).as_ptr(), &mut out, e)).is_err());
            assert!(out.is_null());
        }
        let invalid = [0xff_u8, 0];
        let mut out = ptr::dangling_mut();
        assert!(call(|e| tantivy_ohos_search(h, invalid.as_ptr().cast(), &mut out, e)).is_err());
        assert!(out.is_null());
        assert!(call(|e| tantivy_ohos_commit(ptr::null_mut(), e)).is_err());
        assert!(call(|e| tantivy_ohos_search(h, ptr::null(), ptr::null_mut(), e)).is_err());
        let mut other = ptr::dangling_mut();
        assert!(call(|e| tantivy_ohos_create(
            c(&path).as_ptr(),
            c(&config().to_string()).as_ptr(),
            ptr::null(),
            &mut other,
            e
        ))
        .is_err());
        assert!(other.is_null());
        let rw = OpenOptions {
            struct_size: size_of::<OpenOptions>(),
            mode: 1,
        };
        assert!(
            call(|e| tantivy_ohos_open(c(&path).as_ptr(), &rw, ptr::null(), &mut other, e))
                .is_err()
        );
        assert!(other.is_null());
        tantivy_ohos_close(h);
        std::fs::remove_dir_all(path).unwrap();
        tantivy_ohos_close(ptr::null_mut());
        tantivy_ohos_free_result(ptr::null_mut());
        tantivy_ohos_free_string(ptr::null_mut());
    }
}

#[test]
fn invalid_schema_does_not_create_directory() {
    unsafe {
        for fields in [
            json!([{"name":"id","kind":1,"flags":15}]),
            json!([{"name":"bad name","kind":1,"flags":15}]),
            json!([{"name":"text","kind":99,"flags":15}]),
            json!([{"name":"text","kind":1,"flags":16}]),
            json!([{"name":"text","kind":1,"flags":15},{"name":"text","kind":1,"flags":15}]),
        ] {
            let path = fixture_path("schema");
            let mut h = ptr::dangling_mut();
            assert!(call(|e| tantivy_ohos_create(
                c(&path).as_ptr(),
                c(&json!({"fields":fields}).to_string()).as_ptr(),
                ptr::null(),
                &mut h,
                e
            ))
            .is_err());
            assert!(h.is_null());
            assert!(!Path::new(&path).exists());
        }
    }
}

#[test]
fn boundary_catches_unwinding_panics() {
    unsafe {
        assert!(call(|e| boundary(e, || panic!("test panic")))
            .unwrap_err()
            .contains("panic"));
    }
}

#[test]
fn read_only_metadata_lock_coordinates_with_tantivy_in_both_directions() {
    use std::sync::mpsc;
    use std::time::Duration;
    use tantivy::directory::{Directory, MmapDirectory, META_LOCK};
    let path = tests::fixture_path("metadata-lock");
    unsafe {
        tantivy_ohos_close(create(&path, config()));
    }
    let writable = MmapDirectory::open(&path).unwrap();
    let read_only = readonly::ReadOnlyDirectory::open(&path).unwrap();
    for (first, second) in [
        (
            Box::new(writable.clone()) as Box<dyn Directory>,
            Box::new(read_only.clone()) as Box<dyn Directory>,
        ),
        (
            Box::new(read_only.clone()) as Box<dyn Directory>,
            Box::new(writable.clone()) as Box<dyn Directory>,
        ),
    ] {
        let lock = first.acquire_lock(&META_LOCK).unwrap();
        let (started_tx, started_rx) = mpsc::channel();
        let (acquired_tx, acquired_rx) = mpsc::channel();
        let worker = std::thread::spawn(move || {
            started_tx.send(()).unwrap();
            let lock = second.acquire_lock(&META_LOCK).unwrap();
            acquired_tx.send(()).unwrap();
            drop(lock);
        });
        started_rx.recv_timeout(Duration::from_secs(5)).unwrap();
        assert!(
            matches!(
                acquired_rx.recv_timeout(Duration::from_millis(100)),
                Err(mpsc::RecvTimeoutError::Timeout)
            ),
            "metadata locks must exclude each other"
        );
        drop(lock);
        acquired_rx.recv_timeout(Duration::from_secs(5)).unwrap();
        worker.join().unwrap();
    }
    std::fs::remove_dir_all(path).unwrap();
}
