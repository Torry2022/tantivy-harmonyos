use super::*;

pub(super) fn fixture_path(label: &str) -> String {
    let base = std::env::temp_dir().join(format!(
        "tantivy-ohos-{}-{}-{}",
        label,
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    base.to_str().unwrap().to_owned()
}

fn ids(handle: &SearchHandle, query: &str) -> Vec<String> {
    let hits: serde_json::Value = serde_json::from_str(&handle.search(query, 20).unwrap()).unwrap();
    let mut ids = hits
        .as_array()
        .unwrap()
        .iter()
        .map(|hit| {
            assert!(hit["score"].as_f64().unwrap().is_finite());
            assert!(hit["text"].is_string());
            hit["id"].as_str().unwrap().to_owned()
        })
        .collect::<Vec<_>>();
    ids.sort();
    ids
}

#[test]
fn chinese_queries_mutation_visibility_and_reopen() {
    let path = fixture_path("lifecycle");
    {
        let mut h = SearchHandle::open(&path, true).unwrap();
        h.upsert("a", "鸿蒙设备支持端侧检索与中文资料。型号Qwen3，内存16GB。")
            .unwrap();
        h.upsert("b", "鸿蒙设备提供离线语音。").unwrap();
        h.upsert("c", "咖啡制作指南。").unwrap();
        assert!(ids(&h, "鸿蒙").is_empty());
        h.commit().unwrap();
        assert_eq!(ids(&h, "鸿蒙"), ["a", "b"]);
        assert_eq!(ids(&h, "检索"), ["a"]);
        assert_eq!(ids(&h, "Qwen3"), ["a"]);
        assert_eq!(ids(&h, "16GB"), ["a"]);
        assert_eq!(ids(&h, "鸿蒙 AND 检索"), ["a"]);
        assert_eq!(ids(&h, "\"鸿蒙 设备\""), ["a", "b"]);
        assert!(ids(&h, "xyzneverexists").is_empty());
        assert!(
            SearchHandle::open(&path, false).is_err(),
            "second writer must not acquire lock"
        );
        assert!(
            SearchHandle::open(&path, true).is_err(),
            "create must not overwrite"
        );
    }
    {
        let mut h = SearchHandle::open(&path, false).unwrap();
        assert_eq!(ids(&h, "鸿蒙"), ["a", "b"]);
        h.delete("b").unwrap();
        h.upsert("a", "鸿蒙设备已经更新文档。").unwrap();
        assert_eq!(ids(&h, "检索"), ["a"]);
        h.commit().unwrap();
        assert_eq!(ids(&h, "鸿蒙"), ["a"]);
        assert!(ids(&h, "检索").is_empty());
        assert!(h.search("\"unterminated", 10).is_err());
        assert!(h.search("鸿蒙", 0).is_err());
        h.upsert("uncommitted", "鸿蒙设备").unwrap();
    }
    {
        let h = SearchHandle::open(&path, false).unwrap();
        assert_eq!(ids(&h, "鸿蒙"), ["a"]);
        assert!(ids(&h, "检索").is_empty());
    }
    std::fs::remove_dir_all(path).unwrap();
}

#[test]
fn ffi_rejects_null_and_invalid_utf8() {
    unsafe {
        let mut error = ptr::null_mut();
        assert_eq!(tantivy_ohos_commit(ptr::null_mut(), &mut error), 1);
        assert!(!error.is_null());
        tantivy_ohos_free_string(error);
        let invalid: [u8; 2] = [0xff, 0];
        let mut output = ptr::null_mut();
        assert_eq!(
            tantivy_ohos_open(invalid.as_ptr().cast(), false, &mut output, &mut error),
            1
        );
        assert!(output.is_null());
        tantivy_ohos_free_string(error);
        tantivy_ohos_free_string(ptr::null_mut());
        tantivy_ohos_close(ptr::null_mut());
    }
}
