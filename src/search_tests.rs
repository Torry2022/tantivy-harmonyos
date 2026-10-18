use super::*;
use serde_json::{json, Value as J};

#[test]
fn exact_count_preserves_disjunction_ranks_and_score_bits() {
    unsafe {
        let path = tests::fixture_path("v4-or-count");
        let h = create(&path, &cfg());
        let words = ["alpha", "beta", "gamma", "delta", "epsilon", "zeta"];
        let mut seed = 42_u64;
        for i in 0..180 {
            let mut text = String::new();
            for word in words {
                seed = seed.wrapping_mul(6364136223846793005).wrapping_add(1);
                for _ in 0..(seed >> 32) % 17 {
                    text.push_str(word);
                    text.push(' ');
                }
            }
            put(h, &format!("d{i:03}"), &text, Some(i));
            if i % 30 == 29 {
                call(|e| tantivy_ohos_commit(h, e)).unwrap();
            }
        }
        let mut request = json!({"query":"alpha OR beta OR gamma OR delta OR epsilon OR zeta","limit":100,"projection":"none"});
        let plain = search(h, request.clone());
        request["track_total"] = json!(true);
        let counted = search(h, request);
        assert_eq!(total(plain), (0, false));
        assert_eq!(total(counted), (180, true));
        assert_eq!((*plain).hits.len(), 100);
        for (a, b) in (*plain).hits.iter().zip(&(*counted).hits) {
            assert_eq!(a.id, b.id);
            assert_eq!(a.score.to_bits(), b.score.to_bits());
        }
        tantivy_ohos_free_result(plain);
        tantivy_ohos_free_result(counted);
        tantivy_ohos_close(h);
        std::fs::remove_dir_all(path).unwrap();
    }
}

#[test]
fn public_search_rejects_nonfinite_scores_and_preserves_finite_boosts() {
    unsafe {
        let path = tests::fixture_path("finite-score");
        let h = create(&path, &cfg());
        put(h, "a", "alpha beta", Some(1));
        call(|e| tantivy_ohos_commit(h, e)).unwrap();
        let r = search(h, json!({"query":"alpha","limit":1}));
        let base = (&(*r).hits)[0].score;
        tantivy_ohos_free_result(r);
        let r = search(h, json!({"query":"alpha^2","limit":1}));
        assert_eq!((&(*r).hits)[0].score.to_bits(), (base * 2.0).to_bits());
        tantivy_ohos_free_result(r);
        let large = format!("1{}", "0".repeat(40));
        for count in [false, true] {
            let mut r = ptr::dangling_mut();
            let request = c(
                &json!({"query":format!("alpha^{large}"),"limit":10,"track_total":count})
                    .to_string(),
            );
            assert!(
                call(|e| tantivy_ohos_search(h, request.as_ptr(), &mut r, e))
                    .unwrap_err()
                    .contains("non-finite search score")
            );
            assert!(r.is_null());
        }
        let r = search(
            h,
            json!({"query":format!("alpha^{large}"),"limit":10,"sort":{"field":"n","order":"asc"}}),
        );
        assert_eq!((&(*r).hits)[0].score, 0.0);
        tantivy_ohos_free_result(r);
        tantivy_ohos_close(h);
        std::fs::remove_dir_all(path).unwrap();
    }
}

fn c(s: &str) -> CString {
    CString::new(s).unwrap()
}
unsafe fn call(f: impl FnOnce(*mut *mut c_char) -> i32) -> Result<()> {
    let mut e = ptr::null_mut();
    let status = f(&mut e);
    let message = if e.is_null() {
        String::new()
    } else {
        CStr::from_ptr(e).to_string_lossy().into_owned()
    };
    tantivy_ohos_free_string(e);
    if status == 0 {
        assert!(message.is_empty());
        Ok(())
    } else {
        assert!(!message.is_empty());
        Err(message)
    }
}
fn cfg() -> J {
    json!({"tokenizer":"zh_jieba_v3","fields":[
        {"name":"text","kind":1,"flags":15},{"name":"n","kind":3,"flags":3}],"fast_fields":["n"]})
}
unsafe fn create(path: &str, config: &J) -> *mut SearchHandle {
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
unsafe fn open(path: &str) -> *mut SearchHandle {
    let mut h = ptr::null_mut();
    let options = OpenOptions {
        struct_size: std::mem::size_of::<OpenOptions>(),

        mode: 2,
    };
    call(|e| tantivy_ohos_open(c(path).as_ptr(), &options, ptr::null(), &mut h, e)).unwrap();
    h
}
unsafe fn put(h: *mut SearchHandle, id: &str, text: &str, n: Option<i64>) {
    let names = [c("text"), c("n")];
    let text = c(text);
    let mut values = vec![FieldValue {
        name: names[0].as_ptr(),
        kind: 1,
        string_value: text.as_ptr(),
        i64_value: 0,
        f64_value: 0.0,
    }];
    if let Some(n) = n {
        values.push(FieldValue {
            name: names[1].as_ptr(),
            kind: 3,
            string_value: ptr::null(),
            i64_value: n,
            f64_value: 0.0,
        });
    }
    call(|e| tantivy_ohos_upsert(h, c(id).as_ptr(), values.as_ptr(), values.len(), e)).unwrap();
}
unsafe fn search(h: *mut SearchHandle, r: J) -> *mut SearchResult {
    let mut out = ptr::null_mut();
    call(|e| tantivy_ohos_search(h, c(&r.to_string()).as_ptr(), &mut out, e)).unwrap();
    out
}
unsafe fn total(r: *const SearchResult) -> (u64, bool) {
    let mut n = u64::MAX;
    let mut exact = true;
    call(|e| tantivy_ohos_result_total(r, &mut n, &mut exact, e)).unwrap();
    (n, exact)
}
fn snapshot(path: &Path) -> std::collections::BTreeMap<String, (Vec<u8>, std::time::SystemTime)> {
    std::fs::read_dir(path)
        .unwrap()
        .map(|p| {
            let p = p.unwrap();
            (
                p.file_name().to_string_lossy().into_owned(),
                (
                    std::fs::read(p.path()).unwrap(),
                    p.metadata().unwrap().modified().unwrap(),
                ),
            )
        })
        .collect()
}

#[test]
fn fast_ids_avoid_store_and_preserve_values_and_scores() {
    unsafe {
        let temp = std::path::PathBuf::from(tests::fixture_path("v4-fast"));
        std::fs::create_dir(&temp).unwrap();
        let path = temp.join("fast");
        let path = path.to_str().unwrap();
        let h = create(path, &cfg());
        assert_eq!((*h).config.fast_id, Some(true));
        let large = "中文检索 鸿蒙 星河 ".repeat(10000);
        for i in 0..31 {
            put(h, &format!("编号/é-{i}"), &large, Some(i));
        }
        call(|e| tantivy_ohos_commit(h, e)).unwrap();
        let r = search(
            h,
            json!({"query":"中文检索","limit":31,"projection":"none","track_total":true}),
        );
        assert_eq!(total(r), (31, true));
        assert_eq!((*h).docstore_reads.get(), 0);
        let body = search(
            h,
            json!({"query":"中文检索","limit":31,"projection":"fields","fields":["text"]}),
        );
        assert_eq!((*h).docstore_reads.get(), 31);
        assert_eq!(total(body), (0, false));
        for (a, b) in (*r).hits.iter().zip(&(*body).hits) {
            assert_eq!(a.id, b.id);
            assert_eq!(a.score.to_bits(), b.score.to_bits());
            match &b.values[0].value {
                api::StoredValue::String(s) => assert_eq!(s.to_str().unwrap(), large),
                _ => panic!("type"),
            }
        }
        (*h).docstore_reads.set(0);
        tantivy_ohos_free_result(body);
        tantivy_ohos_close(h);
        let before = snapshot(Path::new(path));
        let ro = open(path);
        let reopened = search(
            ro,
            json!({"query":"中文检索","limit":31,"projection":"none"}),
        );
        assert_eq!((*ro).docstore_reads.get(), 0);
        assert_eq!((&(*r).hits)[0].id, (&(*reopened).hits)[0].id);
        tantivy_ohos_close(ro);
        assert_eq!(snapshot(Path::new(path)), before);
        tantivy_ohos_free_result(reopened);
        assert_eq!(total(r), (31, true));
        tantivy_ohos_free_result(r);
        std::fs::remove_dir_all(temp).unwrap();
    }
}

#[test]
fn count_pages_share_snapshot_and_old_metadata_stays_readable() {
    unsafe {
        let temp = std::path::PathBuf::from(tests::fixture_path("v4-count"));
        std::fs::create_dir(&temp).unwrap();
        for old in [false, true] {
            let path = temp.join(if old { "old-a" } else { "new" });
            let path = path.to_str().unwrap();
            let mut config = cfg();
            if old {
                config["fast_id"] = json!(false);
            }
            let mut h = create(path, &config);
            // A used the same stored-only id schema and omitted the new metadata key.
            if old {
                let p = Path::new(path).join("tantivy-ohos.json");
                let mut saved: J = serde_json::from_slice(&std::fs::read(&p).unwrap()).unwrap();
                saved.as_object_mut().unwrap().remove("fast_id");
                std::fs::write(p, saved.to_string()).unwrap();
            }
            for i in 0..27 {
                put(
                    h,
                    &format!("d{i:02}"),
                    "common 鸿蒙",
                    if i % 3 == 0 { None } else { Some(i % 4) },
                );
            }
            call(|e| tantivy_ohos_commit(h, e)).unwrap();
            tantivy_ohos_close(h);
            let ro = open(path);
            assert_eq!((*ro).config.fast_id, if old { None } else { Some(true) });
            let before = snapshot(Path::new(path));
            for order in ["asc", "desc"] {
                let all = search(
                    ro,
                    json!({"query":"common","limit":100,"projection":"none","sort":{"field":"n","order":order},"track_total":true}),
                );
                assert_eq!(total(all), (27, true));
                for offset in (0..35).step_by(5) {
                    let page = search(
                        ro,
                        json!({"query":"common","limit":5,"offset":offset,"projection":"none","sort":{"field":"n","order":order},"track_total":true}),
                    );
                    assert_eq!(total(page), (27, true));
                    let expected = (*all)
                        .hits
                        .iter()
                        .skip(offset)
                        .take(5)
                        .map(|h| &h.id)
                        .collect::<Vec<_>>();
                    assert_eq!(
                        (*page).hits.iter().map(|h| &h.id).collect::<Vec<_>>(),
                        expected
                    );
                    tantivy_ohos_free_result(page);
                }
                tantivy_ohos_free_result(all);
            }
            assert_eq!(snapshot(Path::new(path)), before);
            assert_eq!((*ro).docstore_reads.get() > 0, old);
            h = Box::into_raw(Box::new(
                SearchHandle::open_config_with_writer(
                    path,
                    None,
                    Some(writer::WriterConfig::default()),
                )
                .unwrap(),
            ));
            put(h, "new", "common 鸿蒙", Some(0));
            call(|e| tantivy_ohos_delete(h, c("d00").as_ptr(), e)).unwrap();
            call(|e| tantivy_ohos_delete(h, c("d01").as_ptr(), e)).unwrap();
            call(|e| tantivy_ohos_commit(h, e)).unwrap();
            let stale = search(ro, json!({"query":"common","limit":3,"track_total":true}));
            assert_eq!(total(stale), (27, true));
            call(|e| tantivy_ohos_reload(ro, e)).unwrap();
            let fresh = search(ro, json!({"query":"common","limit":3,"track_total":true}));
            assert_eq!(total(fresh), (26, true));
            let zero = search(
                ro,
                json!({"query":"nonexistent","limit":3,"track_total":true}),
            );
            assert_eq!(total(zero), (0, true));
            let mut out = ptr::null_mut();
            for bad in [
                json!({"query":"*","limit":0}),
                json!({"query":"*","limit":1,"offset":10000}),
                json!({"query":"*","limit":1,"track_total":"exact"}),
            ] {
                assert!(call(|e| tantivy_ohos_search(
                    ro,
                    c(&bad.to_string()).as_ptr(),
                    &mut out,
                    e
                ))
                .is_err());
                assert!(out.is_null());
            }
            let mut n = 99;
            let mut exact = true;
            assert!(
                call(|e| tantivy_ohos_result_total(ptr::null(), &mut n, &mut exact, e)).is_err()
            );
            assert_eq!((n, exact), (0, false));
            for r in [stale, fresh, zero] {
                tantivy_ohos_free_result(r);
            }
            tantivy_ohos_close(ro);
            tantivy_ohos_close(h);
        }
        std::fs::remove_dir_all(temp).unwrap();
    }
}
