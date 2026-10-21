//! Reads the real published catalogue the way a desktop app does (network): fetch, signature,
//! channel and anti-rollback floor, then offline cache. Ignored by default (needs the network).
#![cfg(not(target_arch = "wasm32"))]

/// Minimal executor: the fetch only waits for a worker thread, no runtime is needed.
fn block_on<T>(future: impl std::future::Future<Output = T>) -> T {
    use std::sync::Arc;
    use std::task::{Context, Poll, Wake, Waker};
    struct Parker(std::thread::Thread);
    impl Wake for Parker {
        fn wake(self: Arc<Self>) {
            self.0.unpark();
        }
    }
    let waker = Waker::from(Arc::new(Parker(std::thread::current())));
    let mut context = Context::from_waker(&waker);
    let mut future = std::pin::pin!(future);
    loop {
        if let Poll::Ready(value) = future.as_mut().poll(&mut context) {
            return value;
        }
        std::thread::park_timeout(std::time::Duration::from_millis(50));
    }
}

#[test]
#[ignore = "network: reads https://kortex.site/registry/catalog.json"]
fn the_desktop_path_fetches_verifies_and_caches_the_published_catalogue() {
    let home = std::env::temp_dir().join(format!("kortex-shell-published-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&home);
    // Only test in this binary: nothing else reads the environment meanwhile.
    unsafe { std::env::set_var("KORTEX_HOME", &home) };
    assert!(
        kortex_shell::catalog::cached().is_none(),
        "empty cache at the start"
    );
    let fetched = block_on(kortex_shell::catalog::fetch()).expect("verified catalogue");
    assert_eq!(fetched.source, kortex_shell::Source::Network);
    assert!(
        fetched.find("code.jwt-decoder").is_some(),
        "a tool added after this build is listed"
    );
    assert!(fetched.len() >= 40, "{}", fetched.len());
    // Offline afterwards: the cache is read back and verified again.
    let cached = kortex_shell::catalog::cached().expect("verified cache");
    assert_eq!(cached.source, kortex_shell::Source::Cached);
    assert_eq!(cached.sequence, fetched.sequence);
    // A tampered cache is refused, never trusted.
    let doc = home.join("cache/public/catalog-stable.json");
    let mut text = std::fs::read_to_string(&doc).expect("cached document");
    text = text.replacen("\"channel\": \"stable\"", "\"channel\": \"stable\" ", 1);
    std::fs::write(&doc, text).expect("tamper");
    assert!(
        kortex_shell::catalog::cached().is_none(),
        "a modified cache fails the signature"
    );
    let _ = std::fs::remove_dir_all(&home);
}
