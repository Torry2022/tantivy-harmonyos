use super::*;
use config::*;
use serde_json::{json, Value as J};
use std::collections::BTreeMap;

fn c(s: &str) -> CString {
    CString::new(s).unwrap()
}
unsafe fn call(f: impl FnOnce(*mut *mut c_char) -> i32) -> Result<()> {
    let mut e = ptr::null_mut();
    let status = f(&mut e);
    let message = if e.is_null() {
        String::new()
    } else {
        CStr::from_ptr(e).to_str().unwrap().to_owned()
    };
    tantivy_ohos_free_string(e);
    if status == 0 {
        assert!(message.is_empty());
        Ok(())
    } else {
        assert_eq!(status, 1);
        assert!(!message.is_empty());
        Err(message)
    }
}
fn config(dict: Option<&str>) -> J {
    let mut j = json!({"version":3,"tokenizer":"zh_jieba_v3","fields":[
        {"name":"title","kind":TEXT,"flags":15},{"name":"n","kind":I64,"flags":3},
        {"name":"x","kind":F64,"flags":3},{"name":"tag","kind":KEYWORD,"flags":3}],"fast_fields":["n","x","tag"]});
    if let Some(word) = dict {
        j["dictionary"] =
            json!({"mode":"extend_default","entries":[{"word":word,"frequency":1000000000}]});
    }
    j
}
unsafe fn create(path: &str, j: &J) -> *mut SearchHandle {
    let mut h = ptr::null_mut();
    call(|e| tantivy_ohos_create_v3(c(path).as_ptr(), c(&j.to_string()).as_ptr(), &mut h, e))
        .unwrap();
    h
}
unsafe fn open(path: &str, mode: u32) -> *mut SearchHandle {
    let mut h = ptr::null_mut();
    let o = OpenOptionsV2 {
        struct_size: std::mem::size_of::<OpenOptionsV2>(),
        version: 2,
        mode,
    };
    call(|e| tantivy_ohos_open_v2(c(path).as_ptr(), &o, &mut h, e)).unwrap();
    h
}
unsafe fn put(
    h: *mut SearchHandle,
    id: &str,
    title: &str,
    n: Option<i64>,
    x: Option<f64>,
    tag: Option<&str>,
) {
    let names = [c("title"), c("n"), c("x"), c("tag")];
    let text = c(title);
    let tag = tag.map(c);
    let mut values = vec![FieldValueV2 {
        name: names[0].as_ptr(),
        kind: TEXT,
        string_value: text.as_ptr(),
        i64_value: 0,
        f64_value: 0.0,
    }];
    if let Some(n) = n {
        values.push(FieldValueV2 {
            name: names[1].as_ptr(),
            kind: I64,
            string_value: ptr::null(),
            i64_value: n,
            f64_value: 0.0,
        });
    }
    if let Some(x) = x {
        values.push(FieldValueV2 {
            name: names[2].as_ptr(),
            kind: F64,
            string_value: ptr::null(),
            i64_value: 0,
            f64_value: x,
        });
    }
    if let Some(tag) = &tag {
        values.push(FieldValueV2 {
            name: names[3].as_ptr(),
            kind: KEYWORD,
            string_value: tag.as_ptr(),
            i64_value: 0,
            f64_value: 0.0,
        });
    }
    call(|e| tantivy_ohos_upsert_v2(h, c(id).as_ptr(), values.as_ptr(), values.len(), e)).unwrap();
}
unsafe fn search(h: *mut SearchHandle, request: J) -> Result<*mut SearchResult> {
    let mut r = ptr::null_mut();
    let result = call(|e| tantivy_ohos_search_v3(h, c(&request.to_string()).as_ptr(), &mut r, e));
    if result.is_err() {
        assert!(r.is_null());
    }
    result.map(|_| r)
}
unsafe fn ids(h: *mut SearchHandle, query: &str) -> Vec<String> {
    let r = search(
        h,
        json!({"version":3,"query":query,"limit":100,"projection":"none"}),
    )
    .unwrap();
    let mut ids = (*r)
        .hits
        .iter()
        .map(|h| h.id.to_str().unwrap().to_owned())
        .collect::<Vec<_>>();
    tantivy_ohos_free_result_v2(r);
    ids.sort();
    ids
}
unsafe fn aggregate(h: *mut SearchHandle, request: J) -> Result<J> {
    let mut out = ptr::null_mut();
    let status =
        call(|e| tantivy_ohos_aggregate_v3(h, c(&request.to_string()).as_ptr(), &mut out, e));
    if let Err(e) = status {
        assert!(out.is_null());
        return Err(e);
    }
    let j = serde_json::from_str(CStr::from_ptr(out).to_str().unwrap()).unwrap();
    tantivy_ohos_free_string(out);
    Ok(j)
}
fn snapshot(path: &str) -> BTreeMap<String, Vec<u8>> {
    std::fs::read_dir(path)
        .unwrap()
        .map(|e| {
            let e = e.unwrap();
            (
                e.file_name().to_string_lossy().into_owned(),
                std::fs::read(e.path()).unwrap(),
            )
        })
        .collect()
}

#[test]
fn dictionary_changes_actual_queries_and_persists_without_external_inputs() {
    let paths = [
        tests::fixture_path("default3"),
        tests::fixture_path("dictionary-a"),
        tests::fixture_path("dictionary-b"),
    ];
    let external =
        std::env::temp_dir().join(format!("dictionary-input-{}.json", std::process::id()));
    unsafe {
        std::fs::write(&external, config(Some("鸿蒙星河检索")).to_string()).unwrap();
        let external_config: J =
            serde_json::from_slice(&std::fs::read(&external).unwrap()).unwrap();
        let hs = [
            create(&paths[0], &config(None)),
            create(&paths[1], &external_config),
            create(&paths[2], &config(Some("端侧向量检索"))),
        ];
        std::fs::remove_file(external).unwrap();
        for h in hs {
            put(h, "a", "鸿蒙星河检索", None, None, None);
            put(h, "b", "鸿蒙 星河 检索", None, None, None);
            call(|e| tantivy_ohos_commit(h, e)).unwrap();
        }
        assert_eq!(ids(hs[0], "鸿蒙星河检索"), ["a", "b"]);
        assert_eq!(ids(hs[1], "鸿蒙星河检索"), ["a"]);
        assert_eq!(ids(hs[2], "鸿蒙星河检索"), ["a", "b"]);
        println!("default/other dictionary: a,b; custom 鸿蒙星河检索: a");
        for h in hs {
            tantivy_ohos_close(h);
        }
        for (i, path) in paths.iter().enumerate() {
            let before = snapshot(path);
            let r = open(path, 2);
            assert!((*r).writer.is_none());
            assert_eq!(
                ids(r, "鸿蒙星河检索"),
                if i == 1 { vec!["a"] } else { vec!["a", "b"] }
            );
            call(|e| tantivy_ohos_reload(r, e)).unwrap();
            tantivy_ohos_close(r);
            assert_eq!(before, snapshot(path));
        }
        let w = open(&paths[1], 1);
        put(w, "c", "鸿蒙星河检索", None, None, None);
        call(|e| tantivy_ohos_commit(w, e)).unwrap();
        assert_eq!(ids(w, "鸿蒙星河检索"), ["a", "c"]);
        tantivy_ohos_close(w);
        let meta = Path::new(&paths[1]).join("tantivy-ohos.json");
        let original = std::fs::read(&meta).unwrap();
        for alteration in 0..4 {
            if alteration == 0 {
                std::fs::remove_file(&meta).unwrap();
            } else {
                let mut j: J = serde_json::from_slice(&original).unwrap();
                match alteration {
                    1 => {
                        j.as_object_mut().unwrap().remove("dictionary");
                    }
                    2 => j["dictionary"]["entries"][0]["word"] = json!("被损坏的词条"),
                    _ => j["dictionary"]["mode"] = json!("replace_default"),
                }
                std::fs::write(&meta, j.to_string()).unwrap();
            }
            let before = snapshot(&paths[1]);
            let o = OpenOptionsV2 {
                struct_size: std::mem::size_of::<OpenOptionsV2>(),
                version: 2,
                mode: 2,
            };
            let mut h = ptr::null_mut();
            assert!(call(|e| tantivy_ohos_open_v2(c(&paths[1]).as_ptr(), &o, &mut h, e)).is_err());
            assert!(h.is_null());
            assert_eq!(before, snapshot(&paths[1]));
            std::fs::write(&meta, &original).unwrap();
        }
    }
    for p in paths {
        std::fs::remove_dir_all(p).unwrap();
    }
}

#[test]
fn native_fast_sort_all_matches_pagination_ties_missing_and_aggregates() {
    let path = tests::fixture_path("sort-agg3");
    unsafe {
        let h = create(&path, &config(None));
        (*h).writer
            .as_mut()
            .unwrap()
            .set_merge_policy(Box::new(tantivy::indexer::NoMergePolicy));
        let mut data = Vec::new();
        let tags = ["中", "a", "Z", "", "é"];
        for i in 0..75 {
            let id = format!("d{i:03}");
            let present = i % 11 != 0;
            let n = present.then_some(if i % 5 == 0 { 7 } else { 37 - i as i64 });
            let x = present.then_some((i as f64 - 37.0) / 4.0);
            let tag = present.then_some(tags[i % 5].to_owned());
            put(h, &id, "common 文档", n, x, tag.as_deref());
            data.push((id, n, x, tag));
            if i % 10 == 9 {
                call(|e| tantivy_ohos_commit(h, e)).unwrap();
            }
        }
        call(|e| tantivy_ohos_commit(h, e)).unwrap();
        put(
            h,
            "low",
            "boundary",
            Some(i64::MIN),
            Some(-f64::MAX),
            Some("A"),
        );
        put(
            h,
            "high",
            "boundary",
            Some(i64::MAX),
            Some(f64::MAX),
            Some("z"),
        );
        call(|e| tantivy_ohos_commit(h, e)).unwrap();
        let searcher = (*h).reader.searcher();
        assert!(searcher.segment_readers().len() >= 8);
        let addresses = searcher
            .search(
                &tantivy::query::AllQuery,
                &tantivy::collector::DocSetCollector,
            )
            .unwrap();
        let mapping: BTreeMap<_, _> = addresses
            .into_iter()
            .map(|a| {
                let d: TantivyDocument = searcher.doc(a).unwrap();
                (
                    d.get_first((*h).id).unwrap().as_str().unwrap().to_owned(),
                    a,
                )
            })
            .collect();
        for field in ["n", "x", "tag"] {
            for direction in ["asc", "desc"] {
                let mut expected = data.clone();
                expected.sort_by(|a, b| {
                    let cmp = match field {
                        "n" => a.1.zip(b.1).map(|(a, b)| a.cmp(&b)),
                        "x" => a.2.zip(b.2).map(|(a, b)| a.partial_cmp(&b).unwrap()),
                        _ => a.3.as_ref().zip(b.3.as_ref()).map(|(a, b)| a.cmp(b)),
                    };
                    let present =
                        |d: &(String, Option<i64>, Option<f64>, Option<String>)| match field {
                            "n" => d.1.is_some(),
                            "x" => d.2.is_some(),
                            _ => d.3.is_some(),
                        };
                    let primary = if let Some(cmp) = cmp {
                        if direction == "asc" {
                            cmp
                        } else {
                            cmp.reverse()
                        }
                    } else {
                        present(b).cmp(&present(a))
                    };
                    primary.then(mapping[&a.0].cmp(&mapping[&b.0]))
                });
                let mut all = Vec::new();
                for offset in (0..80).step_by(7) {
                    let r=search(h,json!({"version":3,"query":"common","limit":7,"offset":offset,"projection":"none","sort":{"field":field,"order":direction}})).unwrap();
                    let actual = (*r)
                        .hits
                        .iter()
                        .map(|h| h.id.to_str().unwrap().to_owned())
                        .collect::<Vec<_>>();
                    let expect = expected
                        .iter()
                        .skip(offset)
                        .take(7)
                        .map(|d| d.0.clone())
                        .collect::<Vec<_>>();
                    assert_eq!(actual, expect, "{field} {direction} offset {offset}");
                    for (i, hit) in (*r).hits.iter().enumerate() {
                        assert_eq!(hit.score, 0.0);
                        assert!(hit.values.is_empty());
                        let mut v = ValueViewV2::default();
                        let mut present = false;
                        call(|e| tantivy_ohos_result_sort_value_v3(r, i, &mut v, &mut present, e))
                            .unwrap();
                        let d = &expected[offset + i];
                        assert_eq!(present, d.1.is_some());
                        match field {
                            "n" => {
                                assert_eq!(v.kind, I64);
                                if present {
                                    assert_eq!(v.i64_value, d.1.unwrap());
                                }
                            }
                            "x" => {
                                assert_eq!(v.kind, F64);
                                if present {
                                    assert_eq!(v.f64_value, d.2.unwrap());
                                }
                            }
                            _ => {
                                assert_eq!(v.kind, KEYWORD);
                                if present {
                                    assert_eq!(
                                        CStr::from_ptr(v.string_value).to_str().unwrap(),
                                        d.3.as_ref().unwrap()
                                    );
                                }
                            }
                        }
                    }
                    all.extend(actual);
                    tantivy_ohos_free_result_v2(r);
                }
                assert_eq!(all.len(), 75);
                println!("sort {field} {direction}: 75 expected rows across 11 pages PASS");
            }
        }
        for (field, direction, expected) in [
            ("n", "asc", "low"),
            ("n", "desc", "high"),
            ("x", "asc", "low"),
            ("x", "desc", "high"),
        ] {
            let r=search(h,json!({"version":3,"query":"boundary","limit":1,"sort":{"field":field,"order":direction}})).unwrap();
            assert_eq!((&(*r).hits)[0].id.to_str().unwrap(), expected);
            tantivy_ohos_free_result_v2(r);
        }
        let request = json!({"version":3,"query":"common","aggregations":{
            "groups":{"terms":{"field":"tag","size":100,"segment_size":4096}},
            "stats":{"stats":{"field":"n"}},"floats":{"stats":{"field":"x"}},
            "ranges":{"range":{"field":"n","ranges":[{"to":0,"key":"negative"},{"from":0,"to":10,"key":"middle"},{"from":10,"key":"high"}]}}
        }});
        let result = aggregate(h, request.clone()).unwrap();
        let aggs = &result["aggregations"];
        let nums: Vec<_> = data.iter().filter_map(|d| d.1).collect();
        let sum: i64 = nums.iter().sum();
        assert_eq!(aggs["stats"]["count"].as_u64().unwrap(), nums.len() as u64);
        assert_eq!(aggs["stats"]["sum"].as_f64().unwrap(), sum as f64);
        assert_eq!(
            aggs["stats"]["min"].as_f64().unwrap(),
            *nums.iter().min().unwrap() as f64
        );
        assert_eq!(
            aggs["stats"]["max"].as_f64().unwrap(),
            *nums.iter().max().unwrap() as f64
        );
        assert_eq!(
            aggs["stats"]["avg"].as_f64().unwrap(),
            sum as f64 / nums.len() as f64
        );
        let floats: Vec<_> = data.iter().filter_map(|d| d.2).collect();
        assert_eq!(
            aggs["floats"]["sum"].as_f64().unwrap(),
            floats.iter().sum::<f64>()
        );
        let mut groups = BTreeMap::<String, u64>::new();
        for (_, _, _, tag) in &data {
            if let Some(tag) = tag {
                *groups.entry(tag.clone()).or_default() += 1;
            }
        }
        let actual: BTreeMap<_, _> = aggs["groups"]["buckets"]
            .as_array()
            .unwrap()
            .iter()
            .map(|b| {
                (
                    b["key"].as_str().unwrap().to_owned(),
                    b["doc_count"].as_u64().unwrap(),
                )
            })
            .collect();
        assert_eq!(actual, groups);
        assert_eq!(aggs["groups"]["doc_count_error_upper_bound"], 0);
        for (key, count) in [
            ("negative", nums.iter().filter(|n| **n < 0).count()),
            (
                "middle",
                nums.iter().filter(|n| **n >= 0 && **n < 10).count(),
            ),
            ("high", nums.iter().filter(|n| **n >= 10).count()),
        ] {
            let b = aggs["ranges"]["buckets"]
                .as_array()
                .unwrap()
                .iter()
                .find(|b| b["key"] == key)
                .unwrap();
            assert_eq!(b["doc_count"], count);
        }
        println!("independent full-match aggregation oracle: {}", result);
        let mut zero = request.clone();
        zero["query"] = json!("nothingmatches");
        let result = aggregate(h, zero).unwrap();
        assert_eq!(result["aggregations"]["stats"]["count"], 0);
        assert_eq!(result["aggregations"]["stats"]["sum"], 0.0);
        assert!(result["aggregations"]["stats"]["avg"].is_null());
        assert_eq!(result["aggregations"]["groups"]["buckets"], json!([]));
        let mut memory = request.clone();
        memory["memory_limit_bytes"] = json!(1);
        let e = aggregate(h, memory).unwrap_err();
        assert!(e.to_lowercase().contains("memory"), "{e}");
        println!("native memory guard rejection: {e}");
        let mut buckets = request.clone();
        buckets["bucket_limit"] = json!(1);
        let e = aggregate(h, buckets).unwrap_err();
        assert!(e.to_lowercase().contains("bucket"), "{e}");
        println!("native bucket guard rejection: {e}");
        tantivy_ohos_close(h);
        let before = snapshot(&path);
        let r = open(&path, 2);
        assert_eq!(
            aggregate(r, request.clone()).unwrap(),
            aggregate(r, request).unwrap()
        );
        assert_eq!(ids(r, "common").len(), 75);
        tantivy_ohos_close(r);
        assert_eq!(before, snapshot(&path));
    }
    std::fs::remove_dir_all(path).unwrap();
}

#[test]
fn v3_rejects_invalid_requests_and_old_indexes_without_fast_fields() {
    unsafe {
        let path = tests::fixture_path("v3-legacy");
        let mut old = ptr::null_mut();
        call(|e| tantivy_ohos_open(c(&path).as_ptr(), true, &mut old, e)).unwrap();
        call(|e| tantivy_ohos_upsert(old, c("a").as_ptr(), c("鸿蒙").as_ptr(), e)).unwrap();
        call(|e| tantivy_ohos_commit(old, e)).unwrap();
        assert_eq!(ids(old, "鸿蒙"), ["a"]);
        let e = search(
            old,
            json!({"version":3,"query":"*","limit":1,"sort":{"field":"text","order":"asc"}}),
        )
        .unwrap_err();
        assert!(e.contains("no fast field"));
        assert!(aggregate(
            old,
            json!({"version":3,"query":"*","aggregations":{"a":{"terms":{"field":"text"}}}})
        )
        .unwrap_err()
        .contains("no fast field"));
        tantivy_ohos_close(old);
        std::fs::remove_dir_all(path).unwrap();
        let path = tests::fixture_path("v3-invalid");
        let h = create(&path, &config(None));
        for j in [
            json!({"version":2,"query":"*","limit":1}),
            json!({"version":3,"query":"*","limit":0}),
            json!({"version":3,"query":"*","limit":1,"offset":10000}),
            json!({"version":3,"query":"*","limit":1,"sort":{"field":"unknown","order":"asc"}}),
            json!({"version":3,"query":"*","limit":1,"sort":{"field":"n","order":"bad"}}),
        ] {
            assert!(search(h, j).is_err());
        }
        for aggs in [
            json!({"a":{"stats":{"field":"tag"}}}),
            json!({"a":{"terms":{"field":"n"}}}),
            json!({"a":{"terms":{"field":"tag","size":5000}}}),
            json!({"a":{"terms":{"field":"tag","segment_size":1}}}),
            json!({"a":{"range":{"field":"n","ranges":[{"from":1,"to":0}]}}}),
            json!({"a":{"range":{"field":"n","ranges":[{"to":2},{"from":1}]}}}),
            json!({"a":{"terms":{"field":"tag"},"aggs":{}}}),
            json!({"a":{"stats":{"field":"unknown"}}}),
        ] {
            assert!(aggregate(h, json!({"version":3,"query":"*","aggregations":aggs})).is_err());
        }
        let mut e = ptr::null_mut();
        assert_eq!(
            tantivy_ohos_create_v3(ptr::null(), ptr::null(), ptr::null_mut(), &mut e),
            1
        );
        tantivy_ohos_free_string(e);
        assert_eq!(
            tantivy_ohos_search_v3(h, ptr::null(), ptr::null_mut(), ptr::null_mut()),
            1
        );
        assert_eq!(
            tantivy_ohos_aggregate_v3(h, ptr::null(), ptr::null_mut(), ptr::null_mut()),
            1
        );
        tantivy_ohos_close(h);
        std::fs::remove_dir_all(path).unwrap();
        for change in 0..5 {
            let path = tests::fixture_path("bad-dict");
            let mut j = config(Some("鸿蒙星河检索"));
            match change {
                0 => j["dictionary"]["mode"] = json!("replace_default"),
                1 => j["dictionary"]["entries"][0]["frequency"] = json!(0),
                2 => j["dictionary"]["entries"][0]["word"] = json!("a b"),
                3 => j["fast_fields"] = json!(["title"]),
                _ => j["dictionary_fingerprint"] = json!("bad"),
            };
            let mut h = ptr::null_mut();
            assert!(call(|e| tantivy_ohos_create_v3(
                c(&path).as_ptr(),
                c(&j.to_string()).as_ptr(),
                &mut h,
                e
            ))
            .is_err());
            assert!(!Path::new(&path).exists());
        }
    }
}

#[test]
fn gapped_ranges_precision_boundaries_and_approximate_terms() {
    unsafe {
        let path = tests::fixture_path("v3-precision");
        let h = create(&path, &config(None));
        for (i, n) in [-10, 0, 5, 10, 15].into_iter().enumerate() {
            put(
                h,
                &format!("s{i}"),
                "small",
                Some(n),
                Some(n as f64 / 2.0),
                None,
            );
        }
        put(h, "missing", "small", None, None, None);
        for (id, n) in [
            ("a", 9_007_199_254_740_992_i64),
            ("b", 9_007_199_254_740_993),
            ("max", i64::MAX),
            ("min", i64::MIN),
        ] {
            put(h, id, "large", Some(n), None, None);
        }
        call(|e| tantivy_ohos_commit(h, e)).unwrap();
        let r=search(h,json!({"version":3,"query":"large","limit":10,"projection":"none","sort":{"field":"n","order":"asc"}})).unwrap();
        assert_eq!(
            (*r).hits
                .iter()
                .map(|h| h.id.to_str().unwrap())
                .collect::<Vec<_>>(),
            ["min", "a", "b", "max"]
        );
        for (i, n) in [
            i64::MIN,
            9_007_199_254_740_992,
            9_007_199_254_740_993,
            i64::MAX,
        ]
        .into_iter()
        .enumerate()
        {
            let mut v = ValueViewV2::default();
            let mut present = false;
            call(|e| tantivy_ohos_result_sort_value_v3(r, i, &mut v, &mut present, e)).unwrap();
            assert!(present);
            assert_eq!(v.i64_value, n);
        }
        tantivy_ohos_free_result_v2(r);
        let request = json!({"version":3,"query":"small","aggregations":{
            "gaps":{"range":{"field":"n","ranges":[{"from":10,"to":15,"key":"high"},{"from":-10,"to":0,"key":"low"}]}},
            "floats":{"range":{"field":"x","ranges":[{"from":-5.0,"to":0.0},{"from":5.0,"to":7.5}]}}
        }});
        let value = aggregate(h, request.clone()).unwrap();
        let b = value["aggregations"]["gaps"]["buckets"].as_array().unwrap();
        assert_eq!(b.len(), 2);
        assert_eq!(b[0]["key"], "high");
        assert_eq!(b[1]["key"], "low");
        assert!(b.iter().all(|b| b["doc_count"] == 1));
        let b = value["aggregations"]["floats"]["buckets"]
            .as_array()
            .unwrap();
        assert_eq!(b.len(), 2);
        assert!(b.iter().all(|b| b["doc_count"] == 1));
        let mut zero = request;
        zero["query"] = json!("absent");
        let value = aggregate(h, zero).unwrap();
        assert!(value["aggregations"]["gaps"]["buckets"]
            .as_array()
            .unwrap()
            .iter()
            .all(|b| b["doc_count"] == 0));
        let large = aggregate(
            h,
            json!({"version":3,"query":"large","aggregations":{"stats":{"stats":{"field":"n"}}}}),
        )
        .unwrap();
        assert_eq!(large["aggregations"]["stats"]["count"], 4);
        assert_eq!(
            large["aggregations"]["stats"]["max"].as_f64().unwrap(),
            i64::MAX as f64
        );
        println!("native stats explicitly f64 at large I64: {large}");
        for endpoint in [
            json!(9_007_199_254_740_993_i64),
            json!(i64::MAX),
            json!(i64::MIN),
            json!(9007199254740992.0),
        ] {
            assert!(aggregate(h,json!({"version":3,"query":"large","aggregations":{"r":{"range":{"field":"n","ranges":[{"from":endpoint}]}}}})).unwrap_err().contains("I64 range"));
        }
        let edge=aggregate(h,json!({"version":3,"query":"large","aggregations":{"r":{"range":{"field":"n","ranges":[{"from":9_007_199_254_740_992_i64}]}}}})).unwrap();
        assert_eq!(edge["aggregations"]["r"]["buckets"][0]["doc_count"], 3);
        put(h, "overflow-a", "overflow", None, Some(f64::MAX), None);
        put(h, "overflow-b", "overflow", None, Some(f64::MAX), None);
        call(|e| tantivy_ohos_commit(h, e)).unwrap();
        let error = aggregate(
            h,
            json!({"version":3,"query":"overflow","aggregations":{"s":{"stats":{"field":"x"}}}}),
        )
        .unwrap_err();
        assert!(error.contains("overflow"), "{error}");
        tantivy_ohos_close(h);
        std::fs::remove_dir_all(path).unwrap();

        let path = tests::fixture_path("v3-term-approx");
        let h = create(&path, &config(None));
        (*h).writer
            .as_mut()
            .unwrap()
            .set_merge_policy(Box::new(tantivy::indexer::NoMergePolicy));
        for s in 0..3 {
            for i in 0..9 {
                put(
                    h,
                    &format!("{s}-{i}"),
                    "common",
                    None,
                    None,
                    Some(if i < 4 { "X" } else { ["A", "B", "C"][s] }),
                );
            }
            call(|e| tantivy_ohos_commit(h, e)).unwrap();
        }
        let req = |segment_size| json!({"version":3,"query":"common","aggregations":{"t":{"terms":{"field":"tag","size":1,"segment_size":segment_size}}}});
        let exact = aggregate(h, req(10)).unwrap();
        assert_eq!(exact["aggregations"]["t"]["buckets"][0]["key"], "X");
        assert_eq!(exact["aggregations"]["t"]["buckets"][0]["doc_count"], 12);
        let approx = aggregate(h, req(1)).unwrap();
        assert!(
            approx["aggregations"]["t"]["doc_count_error_upper_bound"]
                .as_u64()
                .unwrap()
                > 0
        );
        assert!(
            approx["aggregations"]["t"]["sum_other_doc_count"]
                .as_u64()
                .unwrap()
                > 0
        );
        println!("terms candidate truncation visibly approximate: {approx}");
        tantivy_ohos_close(h);
        std::fs::remove_dir_all(path).unwrap();
    }
}
