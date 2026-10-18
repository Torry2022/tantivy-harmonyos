use super::*;
use config::*;
use std::mem::size_of;

fn c(s: &str) -> CString {
    CString::new(s).unwrap()
}
unsafe fn ok(call: impl FnOnce(*mut *mut c_char) -> i32) {
    let mut error = ptr::null_mut();
    let status = call(&mut error);
    let message = if error.is_null() {
        String::new()
    } else {
        CStr::from_ptr(error).to_string_lossy().into_owned()
    };
    tantivy_ohos_free_string(error);
    assert_eq!(status, 0, "{message}");
    assert!(message.is_empty());
}
unsafe fn err(call: impl FnOnce(*mut *mut c_char) -> i32, contains: &str) {
    let mut error = ptr::null_mut();
    assert_eq!(call(&mut error), 1);
    assert!(!error.is_null());
    let message = CStr::from_ptr(error).to_str().unwrap().to_owned();
    tantivy_ohos_free_string(error);
    assert!(
        message.contains(contains),
        "expected {contains:?} in {message:?}"
    );
}
fn options(projection: u32) -> SearchOptionsV2 {
    SearchOptionsV2 {
        struct_size: size_of::<SearchOptionsV2>(),
        version: 2,
        projection,
        fields: ptr::null(),
        field_count: 0,
        query_fields: ptr::null(),
        query_field_count: 0,
    }
}
unsafe fn open(path: &str, mode: u32) -> *mut SearchHandle {
    let o = OpenOptionsV2 {
        struct_size: size_of::<OpenOptionsV2>(),
        version: 2,
        mode,
    };
    let mut h = ptr::null_mut();
    ok(|e| tantivy_ohos_open_v2(c(path).as_ptr(), &o, &mut h, e));
    assert_eq!((*h).writer.is_none(), mode == 2);
    h
}
unsafe fn create(path: &str, specs: &[(&str, u32, u32)]) -> *mut SearchHandle {
    let names: Vec<_> = specs.iter().map(|f| c(f.0)).collect();
    let fields: Vec<_> = specs
        .iter()
        .zip(&names)
        .map(|(f, n)| FieldSpecV2 {
            name: n.as_ptr(),
            kind: f.1,
            flags: f.2,
        })
        .collect();
    let o = CreateOptionsV2 {
        struct_size: size_of::<CreateOptionsV2>(),
        version: 2,
        fields: if fields.is_empty() {
            ptr::null()
        } else {
            fields.as_ptr()
        },
        field_count: fields.len(),
    };
    let mut h = ptr::null_mut();
    ok(|e| tantivy_ohos_create_v2(c(path).as_ptr(), &o, &mut h, e));
    h
}
unsafe fn search(h: *mut SearchHandle, q: &str, o: &SearchOptionsV2) -> *mut SearchResult {
    let mut result = ptr::null_mut();
    ok(|e| tantivy_ohos_search_v2(h, c(q).as_ptr(), 20, o, &mut result, e));
    assert!(!result.is_null());
    result
}
unsafe fn count(r: *mut SearchResult) -> usize {
    let mut n = 999;
    ok(|e| tantivy_ohos_result_count_v2(r, &mut n, e));
    n
}
unsafe fn hit(r: *mut SearchResult, i: usize) -> HitViewV2 {
    let mut v = HitViewV2::default();
    ok(|e| tantivy_ohos_result_hit_v2(r, i, &mut v, e));
    assert!(v.score.is_finite());
    v
}
unsafe fn value(r: *mut SearchResult, h: usize, f: usize) -> ValueViewV2 {
    let mut v = ValueViewV2::default();
    ok(|e| tantivy_ohos_result_value_v2(r, h, f, &mut v, e));
    v
}
unsafe fn ids(h: *mut SearchHandle, q: &str, o: &SearchOptionsV2) -> Vec<String> {
    let r = search(h, q, o);
    let mut ids: Vec<_> = (0..count(r))
        .map(|i| CStr::from_ptr(hit(r, i).id).to_str().unwrap().to_owned())
        .collect();
    tantivy_ohos_free_result_v2(r);
    ids.sort();
    ids
}
fn sv(name: &CString, kind: u32, text: &CString) -> FieldValueV2 {
    FieldValueV2 {
        name: name.as_ptr(),
        kind,
        string_value: text.as_ptr(),
        i64_value: 0,
        f64_value: 0.0,
    }
}
fn snapshot(path: &str) -> Vec<(String, Vec<u8>, std::time::SystemTime)> {
    let mut files: Vec<_> = std::fs::read_dir(path)
        .unwrap()
        .map(|e| {
            let e = e.unwrap();
            let meta = e.metadata().unwrap();
            assert!(meta.is_file());
            (
                e.file_name().to_string_lossy().into_owned(),
                std::fs::read(e.path()).unwrap(),
                meta.modified().unwrap(),
            )
        })
        .collect();
    files.sort_by(|a, b| a.0.cmp(&b.0));
    files
}

#[test]
fn public_multifield_lifecycle_projection_and_readers() {
    let path = tests::fixture_path("v2-lifecycle");
    unsafe {
        let w = create(
            &path,
            &[
                ("title", TEXT, STORED_FLAG | INDEXED | QUERY | RETURN),
                ("body", TEXT, INDEXED),
                ("tag", KEYWORD, STORED_FLAG | INDEXED),
                ("year", I64, STORED_FLAG | INDEXED | RETURN),
                ("rating", F64, STORED_FLAG | INDEXED),
                ("note", TEXT, STORED_FLAG),
                ("optional", TEXT, STORED_FLAG | RETURN),
            ],
        );
        let (title, body, tag, year, rating, note) = (
            c("title"),
            c("body"),
            c("tag"),
            c("year"),
            c("rating"),
            c("note"),
        );
        let (text, body_text, tag_text, empty) = (
            c("鸿蒙设备支持中文检索 Qwen3 16GB"),
            c("咖啡制作指南"),
            c("Guide-A"),
            c(""),
        );
        let vals = [
            sv(&title, TEXT, &text),
            sv(&body, TEXT, &body_text),
            sv(&tag, KEYWORD, &tag_text),
            FieldValueV2 {
                name: year.as_ptr(),
                kind: I64,
                string_value: ptr::null(),
                i64_value: 2026,
                f64_value: 0.0,
            },
            FieldValueV2 {
                name: rating.as_ptr(),
                kind: F64,
                string_value: ptr::null(),
                i64_value: 0,
                f64_value: 4.5,
            },
            sv(&note, TEXT, &empty),
        ];
        ok(|e| tantivy_ohos_upsert_v2(w, c("a").as_ptr(), vals.as_ptr(), vals.len(), e));
        let defaults = options(0);
        let none = options(1);
        assert!(ids(w, "鸿蒙", &defaults).is_empty());
        let ro1 = open(&path, 2);
        let ro2 = open(&path, 2);
        let rw = OpenOptionsV2 {
            struct_size: size_of::<OpenOptionsV2>(),
            version: 2,
            mode: 1,
        };
        let mut rejected = ptr::null_mut();
        err(
            |e| tantivy_ohos_open_v2(c(&path).as_ptr(), &rw, &mut rejected, e),
            "lock",
        );
        assert!(rejected.is_null());
        ok(|e| tantivy_ohos_commit(w, e));
        assert_eq!(ids(w, "鸿蒙 检索", &defaults), ["a"]);
        assert_eq!(ids(w, "\"鸿蒙 设备\"", &none), ["a"]);
        assert_eq!(ids(w, "Qwen3 AND 16GB", &none), ["a"]);
        assert!(ids(ro1, "鸿蒙", &none).is_empty());
        ok(|e| tantivy_ohos_reload(ro1, e));
        assert_eq!(ids(ro1, "鸿蒙", &none), ["a"]);
        assert!(ids(ro2, "鸿蒙", &none).is_empty());
        assert!(ids(w, "咖啡", &none).is_empty());
        assert_eq!(ids(w, "body:咖啡", &none), ["a"]);
        assert_eq!(ids(w, "tag:Guide-A", &none), ["a"]);
        assert!(ids(w, "tag:guide-a", &none).is_empty());
        assert_eq!(ids(w, "year:[2020 TO 2026]", &none), ["a"]);
        assert_eq!(ids(w, "rating:[4 TO 5]", &none), ["a"]);
        let qf = [body.as_ptr()];
        let mut override_o = options(1);
        override_o.query_fields = qf.as_ptr();
        override_o.query_field_count = 1;
        assert_eq!(ids(w, "咖啡", &override_o), ["a"]);
        let r = search(w, "鸿蒙", &defaults);
        assert_eq!(count(r), 1);
        assert_eq!(hit(r, 0).field_count, 2); // absent optional omitted
        assert_eq!(
            CStr::from_ptr(value(r, 0, 0).name).to_str().unwrap(),
            "title"
        );
        assert_eq!(
            CStr::from_ptr(value(r, 0, 0).string_value)
                .to_str()
                .unwrap(),
            text.to_str().unwrap()
        );
        let n = value(r, 0, 1);
        assert_eq!(n.kind, I64);
        assert_eq!(n.i64_value, 2026);
        assert!(n.string_value.is_null());
        tantivy_ohos_free_result_v2(r);
        let r = search(w, "鸿蒙", &none);
        assert_eq!(hit(r, 0).field_count, 0);
        tantivy_ohos_free_result_v2(r);
        let fields = [tag.as_ptr(), rating.as_ptr(), note.as_ptr()];
        let mut explicit = options(2);
        explicit.fields = fields.as_ptr();
        explicit.field_count = fields.len();
        let retained = search(w, "鸿蒙", &explicit);
        assert_eq!(hit(retained, 0).field_count, 3);
        assert_eq!(
            CStr::from_ptr(value(retained, 0, 0).string_value)
                .to_str()
                .unwrap(),
            "Guide-A"
        );
        assert_eq!(value(retained, 0, 1).f64_value, 4.5);
        assert_eq!(
            CStr::from_ptr(value(retained, 0, 2).string_value).to_bytes(),
            b""
        );
        err(|e| tantivy_ohos_commit(ro1, e), "read-only");
        err(
            |e| tantivy_ohos_delete(ro1, c("a").as_ptr(), e),
            "read-only",
        );
        err(
            |e| tantivy_ohos_upsert_v2(ro1, c("a").as_ptr(), ptr::null(), 0, e),
            "read-only",
        );
        err(
            |e| tantivy_ohos_upsert(w, c("a").as_ptr(), text.as_ptr(), e),
            "legacy",
        );
        let mut json = ptr::null_mut();
        err(
            |e| tantivy_ohos_search(w, text.as_ptr(), 10, &mut json, e),
            "legacy",
        );
        assert!(json.is_null());
        ok(|e| tantivy_ohos_config_json_v2(ro1, &mut json, e));
        let config: serde_json::Value =
            serde_json::from_str(CStr::from_ptr(json).to_str().unwrap()).unwrap();
        assert_eq!(config["fields"].as_array().unwrap().len(), 7);
        tantivy_ohos_free_string(json);
        // Replace twice in a batch sequence; omitted fields disappear.
        ok(|e| tantivy_ohos_upsert_v2(w, c("a").as_ptr(), vals.as_ptr(), vals.len(), e));
        let updated = c("离线语音更新");
        let replacement = [sv(&title, TEXT, &updated)];
        ok(|e| tantivy_ohos_upsert_v2(w, c("a").as_ptr(), replacement.as_ptr(), 1, e));
        ok(|e| tantivy_ohos_upsert_v2(w, c("b").as_ptr(), vals.as_ptr(), vals.len(), e));
        ok(|e| tantivy_ohos_commit(w, e));
        assert_eq!(ids(w, "鸿蒙", &none), ["b"]);
        assert_eq!(ids(w, "语音", &none), ["a"]);
        let r = search(w, "语音", &explicit);
        assert_eq!(hit(r, 0).field_count, 0);
        tantivy_ohos_free_result_v2(r);
        ok(|e| tantivy_ohos_delete(w, c("b").as_ptr(), e));
        assert_eq!(ids(w, "鸿蒙", &none), ["b"]);
        ok(|e| tantivy_ohos_commit(w, e));
        ok(|e| tantivy_ohos_upsert_v2(w, c("uncommitted").as_ptr(), vals.as_ptr(), vals.len(), e));
        tantivy_ohos_close(w);
        tantivy_ohos_close(ro1);
        tantivy_ohos_close(ro2);
        // Result owns its strings and survives commits, reload and handle close.
        assert_eq!(CStr::from_ptr(hit(retained, 0).id).to_str().unwrap(), "a");
        assert_eq!(value(retained, 0, 1).f64_value, 4.5);
        tantivy_ohos_free_result_v2(retained);
        let w = open(&path, 1);
        assert_eq!(ids(w, "语音", &none), ["a"]);
        assert!(ids(w, "鸿蒙", &none).is_empty());
        tantivy_ohos_close(w);
        let before = snapshot(&path);
        let ro1 = open(&path, 2);
        let ro2 = open(&path, 2);
        assert_eq!(ids(ro1, "语音", &none), ["a"]);
        ok(|e| tantivy_ohos_reload(ro2, e));
        tantivy_ohos_close(ro1);
        tantivy_ohos_close(ro2);
        assert_eq!(
            before,
            snapshot(&path),
            "read-only lifecycle must not change file set, bytes or modification times"
        );
    }
    std::fs::remove_dir_all(path).unwrap();
}

#[test]
fn legacy_without_metadata_and_read_only_missing_lock() {
    let path = tests::fixture_path("legacy-no-metadata");
    {
        let mut h = SearchHandle::open(&path, true).unwrap();
        h.upsert("a", "鸿蒙设备检索").unwrap();
        h.commit().unwrap();
    }
    let metadata = Path::new(&path).join("tantivy-ohos.json");
    std::fs::remove_file(&metadata).unwrap(); // exact original on-disk schema
    unsafe {
        let before = snapshot(&path);
        let r = open(&path, 2);
        let mut json = ptr::null_mut();
        ok(|e| tantivy_ohos_search(r, c("鸿蒙").as_ptr(), 10, &mut json, e));
        let hits: serde_json::Value =
            serde_json::from_str(CStr::from_ptr(json).to_str().unwrap()).unwrap();
        assert_eq!(hits[0]["id"], "a");
        tantivy_ohos_free_string(json);
        err(
            |e| tantivy_ohos_upsert(r, c("a").as_ptr(), c("更新").as_ptr(), e),
            "read-only",
        );
        tantivy_ohos_close(r);
        assert_eq!(before, snapshot(&path));
        let lock = Path::new(&path).join(".tantivy-meta.lock");
        std::fs::remove_file(&lock).unwrap();
        let o = OpenOptionsV2 {
            struct_size: size_of::<OpenOptionsV2>(),
            version: 2,
            mode: 2,
        };
        let mut h = ptr::null_mut();
        err(
            |e| tantivy_ohos_open_v2(c(&path).as_ptr(), &o, &mut h, e),
            "",
        );
        assert!(h.is_null());
        assert!(!lock.exists());
        assert!(!metadata.exists());
        let h = open(&path, 1);
        tantivy_ohos_close(h);
        assert!(lock.exists());
        let mut legacy = ptr::null_mut();
        ok(|e| tantivy_ohos_open(c(&path).as_ptr(), false, &mut legacy, e));
        assert_eq!(ids(legacy, "鸿蒙", &options(1)), ["a"]);
        tantivy_ohos_close(legacy);
    }
    std::fs::remove_dir_all(path).unwrap();
}

#[test]
fn metadata_is_validated_on_reopen() {
    let path = tests::fixture_path("metadata");
    unsafe {
        let h = create(&path, &[("title", TEXT, STORED_FLAG | INDEXED | QUERY)]);
        tantivy_ohos_close(h);
        let metadata = Path::new(&path).join("tantivy-ohos.json");
        let bytes = std::fs::read(&metadata).unwrap();
        let config: Config = serde_json::from_slice(&bytes).unwrap();
        let o = OpenOptionsV2 {
            struct_size: size_of::<OpenOptionsV2>(),
            version: 2,
            mode: 2,
        };
        for altered in [
            "{broken".into(),
            String::from_utf8(bytes.clone())
                .unwrap()
                .replace("zh_jieba_v1", "unknown"),
            {
                let mut c = config;
                c.fields[0].flags = STORED_FLAG;
                serde_json::to_string(&c).unwrap()
            },
        ] {
            std::fs::write(&metadata, altered).unwrap();
            let mut h = ptr::null_mut();
            let before = snapshot(&path);
            err(
                |e| tantivy_ohos_open_v2(c(&path).as_ptr(), &o, &mut h, e),
                "",
            );
            assert!(h.is_null());
            assert_eq!(before, snapshot(&path));
        }
        std::fs::remove_file(&metadata).unwrap();
        let mut h = ptr::null_mut();
        err(
            |e| tantivy_ohos_open_v2(c(&path).as_ptr(), &o, &mut h, e),
            "metadata",
        );
        std::fs::write(&metadata, &bytes).unwrap();
        let h = open(&path, 2);
        tantivy_ohos_close(h);
    }
    std::fs::remove_dir_all(path).unwrap();
}

#[test]
fn invalid_arguments_leave_documents_and_outputs_intact() {
    let path = tests::fixture_path("invalid-v2");
    unsafe {
        let name = c("title");
        let text = c("鸿蒙检索");
        let w = create(
            &path,
            &[("title", TEXT, STORED_FLAG | INDEXED | QUERY | RETURN)],
        );
        let good = [sv(&name, TEXT, &text)];
        ok(|e| tantivy_ohos_upsert_v2(w, c("a").as_ptr(), good.as_ptr(), 1, e));
        ok(|e| tantivy_ohos_commit(w, e));
        let invalid_utf8 = [0xff_u8, 0];
        let bad_utf8 = [FieldValueV2 {
            string_value: invalid_utf8.as_ptr().cast(),
            ..sv(&name, TEXT, &text)
        }];
        let unknown = c("unknown");
        let bad_name = [sv(&unknown, TEXT, &text)];
        let wrong_type = [sv(&name, KEYWORD, &text)];
        let duplicate = [sv(&name, TEXT, &text), sv(&name, TEXT, &text)];
        for (values, label) in [
            (&bad_utf8[..], "utf-8"),
            (&bad_name[..], "unknown"),
            (&wrong_type[..], "type mismatch"),
            (&duplicate[..], "duplicate"),
        ] {
            err(
                |e| tantivy_ohos_upsert_v2(w, c("a").as_ptr(), values.as_ptr(), values.len(), e),
                label,
            );
        }
        err(
            |e| tantivy_ohos_upsert_v2(w, c("").as_ptr(), ptr::null(), 0, e),
            "empty",
        );
        err(
            |e| tantivy_ohos_upsert_v2(w, invalid_utf8.as_ptr().cast(), ptr::null(), 0, e),
            "utf-8",
        );
        err(
            |e| tantivy_ohos_upsert_v2(w, c("a").as_ptr(), ptr::null(), 1, e),
            "null array",
        );
        err(
            |e| tantivy_ohos_upsert_v2(w, c("a").as_ptr(), good.as_ptr(), 0, e),
            "NULL/0",
        );
        err(|e| tantivy_ohos_delete(w, c("").as_ptr(), e), "empty");
        ok(|e| tantivy_ohos_commit(w, e));
        assert_eq!(
            ids(w, "鸿蒙", &options(1)),
            ["a"],
            "failed validation must not queue a delete"
        );
        let field = [name.as_ptr()];
        let unknown_f = [unknown.as_ptr()];
        let dup = [name.as_ptr(), name.as_ptr()];
        let mut bad_options = vec![options(2), options(99)];
        let mut o = options(0);
        o.fields = field.as_ptr();
        o.field_count = 1;
        bad_options.push(o);
        let mut o = options(1);
        o.field_count = 1;
        bad_options.push(o);
        let mut o = options(2);
        o.fields = unknown_f.as_ptr();
        o.field_count = 1;
        bad_options.push(o);
        let mut o = options(2);
        o.fields = dup.as_ptr();
        o.field_count = 2;
        bad_options.push(o);
        let mut o = options(1);
        o.query_fields = unknown_f.as_ptr();
        o.query_field_count = 1;
        bad_options.push(o);
        let mut o = options(1);
        o.query_fields = dup.as_ptr();
        o.query_field_count = 2;
        bad_options.push(o);
        let mut o = options(1);
        o.version = 99;
        bad_options.push(o);
        let mut o = options(1);
        o.struct_size -= 1;
        bad_options.push(o);
        for o in &bad_options {
            let mut r = ptr::null_mut();
            err(
                |e| tantivy_ohos_search_v2(w, text.as_ptr(), 10, o, &mut r, e),
                "",
            );
            assert!(r.is_null());
        }
        let o = options(1);
        for limit in [0, 1001] {
            let mut r = ptr::null_mut();
            err(
                |e| tantivy_ohos_search_v2(w, text.as_ptr(), limit, &o, &mut r, e),
                "limit",
            );
            assert!(r.is_null());
        }
        let mut r = ptr::null_mut();
        err(
            |e| tantivy_ohos_search_v2(w, text.as_ptr(), 10, ptr::null(), &mut r, e),
            "null",
        );
        err(
            |e| tantivy_ohos_search_v2(w, invalid_utf8.as_ptr().cast(), 10, &o, &mut r, e),
            "utf-8",
        );
        err(
            |e| tantivy_ohos_search_v2(w, c("\"unterminated").as_ptr(), 10, &o, &mut r, e),
            "",
        );
        err(
            |e| tantivy_ohos_search_v2(w, text.as_ptr(), 10, &o, ptr::null_mut(), e),
            "null",
        );
        let r = search(w, "鸿蒙", &o);
        let mut view = hit(r, 0);
        err(|e| tantivy_ohos_result_hit_v2(r, 1, &mut view, e), "range");
        assert!(view.id.is_null());
        assert_eq!(view.field_count, 0);
        let mut val = ValueViewV2::default();
        err(
            |e| tantivy_ohos_result_value_v2(r, 0, 0, &mut val, e),
            "range",
        );
        assert!(val.name.is_null());
        err(
            |e| tantivy_ohos_result_value_v2(r, 1, 0, &mut val, e),
            "range",
        );
        let mut n = 9;
        err(
            |e| tantivy_ohos_result_count_v2(ptr::null(), &mut n, e),
            "null",
        );
        assert_eq!(n, 0);
        err(
            |e| tantivy_ohos_result_count_v2(r, ptr::null_mut(), e),
            "null",
        );
        err(
            |e| tantivy_ohos_result_hit_v2(r, 0, ptr::null_mut(), e),
            "null",
        );
        err(
            |e| tantivy_ohos_result_value_v2(r, 0, 0, ptr::null_mut(), e),
            "null",
        );
        tantivy_ohos_free_result_v2(r);
        let empty = search(w, "xyzneverexists", &o);
        assert_eq!(count(empty), 0);
        tantivy_ohos_free_result_v2(empty);
        let mut json = ptr::null_mut();
        err(
            |e| tantivy_ohos_config_json_v2(ptr::null_mut(), &mut json, e),
            "null",
        );
        assert!(json.is_null());
        err(|e| tantivy_ohos_reload(ptr::null_mut(), e), "null");
        assert_eq!(tantivy_ohos_commit(ptr::null_mut(), ptr::null_mut()), 1);
        tantivy_ohos_free_result_v2(ptr::null_mut());
        tantivy_ohos_free_string(ptr::null_mut());
        tantivy_ohos_close(ptr::null_mut());
        tantivy_ohos_close(w);
    }
    std::fs::remove_dir_all(path).unwrap();
}

#[test]
fn schema_options_numeric_values_and_empty_defaults() {
    unsafe {
        let path = tests::fixture_path("invalid-schema");
        let mut h = ptr::null_mut();
        let mut create_o = CreateOptionsV2 {
            struct_size: size_of::<CreateOptionsV2>(),
            version: 2,
            fields: ptr::null(),
            field_count: 0,
        };
        create_o.version = 1;
        err(
            |e| tantivy_ohos_create_v2(c(&path).as_ptr(), &create_o, &mut h, e),
            "version",
        );
        create_o.version = 2;
        create_o.struct_size = 0;
        err(
            |e| tantivy_ohos_create_v2(c(&path).as_ptr(), &create_o, &mut h, e),
            "struct_size",
        );
        create_o.struct_size = size_of::<CreateOptionsV2>();
        create_o.field_count = 65;
        err(
            |e| tantivy_ohos_create_v2(c(&path).as_ptr(), &create_o, &mut h, e),
            "64",
        );
        let names = [c("id"), c("bad-name"), c(""), c("title")];
        for (name, kind, flags) in [
            (&names[0], TEXT, 1),
            (&names[1], TEXT, 1),
            (&names[2], TEXT, 1),
            (&names[3], 99, 1),
            (&names[3], TEXT, 0),
            (&names[3], TEXT, 16),
            (&names[3], TEXT, INDEXED | RETURN),
            (&names[3], TEXT, STORED_FLAG | QUERY),
            (&names[3], I64, INDEXED | QUERY),
        ] {
            let f = [FieldSpecV2 {
                name: name.as_ptr(),
                kind,
                flags,
            }];
            create_o.fields = f.as_ptr();
            create_o.field_count = 1;
            err(
                |e| tantivy_ohos_create_v2(c(&path).as_ptr(), &create_o, &mut h, e),
                "",
            );
            assert!(h.is_null());
            assert!(
                !Path::new(&path).exists(),
                "invalid schema must be rejected before creating directory"
            );
        }
        let duplicate = [
            FieldSpecV2 {
                name: names[3].as_ptr(),
                kind: TEXT,
                flags: 1,
            },
            FieldSpecV2 {
                name: names[3].as_ptr(),
                kind: TEXT,
                flags: 1,
            },
        ];
        create_o.fields = duplicate.as_ptr();
        create_o.field_count = 2;
        err(
            |e| tantivy_ohos_create_v2(c(&path).as_ptr(), &create_o, &mut h, e),
            "duplicate",
        );
        err(
            |e| tantivy_ohos_create_v2(c(&path).as_ptr(), ptr::null(), &mut h, e),
            "null",
        );
        let bad_o = OpenOptionsV2 {
            struct_size: size_of::<OpenOptionsV2>(),
            version: 2,
            mode: 9,
        };
        err(
            |e| tantivy_ohos_open_v2(c(&path).as_ptr(), &bad_o, &mut h, e),
            "mode",
        );
        assert!(!Path::new(&path).exists());
        // No default query/return fields: ID-qualified query works, default projection is empty.
        let w = create(
            &path,
            &[
                ("number", F64, STORED_FLAG | INDEXED),
                ("hidden", TEXT, INDEXED),
            ],
        );
        let name = c("number");
        for n in [f64::NAN, f64::INFINITY, f64::NEG_INFINITY] {
            let v = [FieldValueV2 {
                name: name.as_ptr(),
                kind: F64,
                string_value: ptr::null(),
                i64_value: 0,
                f64_value: n,
            }];
            err(
                |e| tantivy_ohos_upsert_v2(w, c("a").as_ptr(), v.as_ptr(), 1, e),
                "finite",
            );
        }
        ok(|e| tantivy_ohos_upsert_v2(w, c("a").as_ptr(), ptr::null(), 0, e));
        ok(|e| tantivy_ohos_commit(w, e));
        let r = search(w, "id:a", &options(0));
        assert_eq!(count(r), 1);
        assert_eq!(hit(r, 0).field_count, 0);
        tantivy_ohos_free_result_v2(r);
        let hidden = c("hidden");
        let fields = [hidden.as_ptr()];
        let mut o = options(2);
        o.fields = fields.as_ptr();
        o.field_count = 1;
        let mut r = ptr::null_mut();
        err(
            |e| tantivy_ohos_search_v2(w, c("id:a").as_ptr(), 10, &o, &mut r, e),
            "not stored",
        );
        let fields = [name.as_ptr()];
        let mut o = options(1);
        o.query_fields = fields.as_ptr();
        o.query_field_count = 1;
        err(
            |e| tantivy_ohos_search_v2(w, c("id:a").as_ptr(), 10, &o, &mut r, e),
            "indexed text/keyword",
        );
        tantivy_ohos_close(w);
        std::fs::remove_dir_all(path).unwrap();
    }
}

#[test]
fn boundary_catches_unwinding_panics() {
    unsafe {
        err(
            |e| {
                boundary(e, || -> Result<()> {
                    panic!("test panic");
                })
            },
            "Rust panic",
        );
    }
}

#[test]
fn read_only_metadata_lock_coordinates_with_tantivy_in_both_directions() {
    use std::sync::mpsc;
    use std::time::Duration;
    use tantivy::directory::{Directory, MmapDirectory, META_LOCK};
    let path = tests::fixture_path("metadata-lock");
    drop(SearchHandle::open(&path, true).unwrap());
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
