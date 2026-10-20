#![cfg(not(target_arch = "wasm32"))]
use kortex_platform::{
    Storage,
    storage::{MAX_VALUE_BYTES, NativeStorage},
};

#[test]
fn preferences_persist_are_isolated_bounded_and_replaced_without_partial_files() {
    let root = std::env::temp_dir().join(format!(
        "kortex-storage-{}-{}",
        std::process::id(),
        kortex_platform::now_ms()
    ));
    let mut first = NativeStorage::open(&root, "desk/../é").expect("open");
    let second = NativeStorage::open(&root, "desk").expect("other namespace");
    assert_eq!(first.get("theme").expect("missing"), None);
    first.set("theme", "dark 🌙").expect("write");
    let mut reopened = NativeStorage::open(&root, "desk/../é").expect("reopen");
    assert_eq!(
        reopened.get("theme").expect("persisted"),
        Some("dark 🌙".into())
    );
    assert_eq!(second.get("theme").expect("isolated"), None);
    reopened.set("theme", "").expect("empty string");
    assert_eq!(first.get("theme").expect("replaced"), Some(String::new()));
    first.set("../outside", "confined").expect("encoded key");
    assert!(!root.join("outside").exists());
    let max = "x".repeat(MAX_VALUE_BYTES);
    first.set("limit", &max).expect("limit");
    assert!(first.set("limit", &(max.clone() + "x")).is_err());
    assert_eq!(
        first.get("limit").expect("unchanged after rejected write"),
        Some(max)
    );
    assert!(first.set("", "x").is_err());
    assert!(first.get(&"é".repeat(65)).is_err());
    assert!(NativeStorage::open(&root, "").is_err());
    reopened.remove("theme").expect("delete");
    reopened.remove("theme").expect("idempotent");
    assert_eq!(first.get("theme").expect("removed"), None);
    for namespace in std::fs::read_dir(&root).expect("namespaces") {
        for entry in std::fs::read_dir(namespace.expect("directory").path()).expect("keys") {
            assert!(
                !entry
                    .expect("file")
                    .file_name()
                    .to_string_lossy()
                    .starts_with(".pending-")
            );
        }
    }
    std::fs::remove_dir_all(&root).expect("cleanup");
}

#[test]
fn unreadable_corrupt_and_oversized_entries_are_explicit_errors() {
    let root = std::env::temp_dir().join(format!("kortex-storage-errors-{}", std::process::id()));
    let mut store = NativeStorage::open(&root, "test").expect("open");
    store.set("key", "ok").expect("write");
    let directory = root.join("74657374");
    let path = directory.join("6b6579");
    std::fs::write(&path, [255]).expect("invalid UTF8");
    assert!(store.get("key").is_err());
    std::fs::write(&path, vec![b'x'; MAX_VALUE_BYTES + 1]).expect("oversize fixture");
    assert!(store.get("key").is_err());
    std::fs::remove_file(&path).expect("remove fixture");
    std::fs::create_dir(&path).expect("wrong type");
    assert!(store.get("key").is_err());
    assert!(store.set("key", "replacement").is_err());
    assert_eq!(
        std::fs::read_dir(&directory).expect("no temp leak").count(),
        1
    );
    #[cfg(unix)]
    {
        let link = directory.join("6c696e6b");
        std::os::unix::fs::symlink(&path, &link).expect("link fixture");
        assert!(store.get("link").is_err());
    }
    std::fs::remove_dir_all(&root).expect("cleanup");
    std::fs::write(&root, "not a directory").expect("unavailable fixture");
    assert!(NativeStorage::open(&root, "test").is_err());
    std::fs::remove_file(&root).expect("cleanup");
}
