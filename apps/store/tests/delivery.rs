//! Desktop delivery against the real signed production index (network): `cargo test -p kortex-store --test delivery -- --ignored`.
#![cfg(not(target_arch = "wasm32"))]

use kortex_store::delivery::{perform, snapshot};
use kortex_store::engine::{Action, Outcome};

#[test]
#[ignore = "network: reads https://kortex.site/registry/index.json"]
fn the_store_reads_the_signed_index_and_refuses_installs_without_an_account() {
    let dir = std::env::temp_dir().join(format!("kortex-store-delivery-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    // SAFETY: single-threaded test binary; no other thread reads the environment yet.
    unsafe { std::env::set_var("KORTEX_HOME", &dir) };
    let snap = snapshot();
    assert_eq!(snap.error, None);
    assert!(snap.items.len() >= 44, "{}", snap.items.len());
    assert!(
        snap.items
            .iter()
            .any(|s| s.id == "apps.hub" && s.universe == "apps")
    );
    assert!(
        snap.items
            .iter()
            .all(|s| s.installed.is_none() && s.supported)
    );
    if snap.account.is_none() {
        assert_eq!(
            perform("calc.percentage", Action::Install, None),
            Outcome::SignedOut
        );
        assert!(!dir.join("tools/calc.percentage").exists());
    }
    let _ = std::fs::remove_dir_all(&dir);
}
