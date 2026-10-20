//! Desktop discovery (RULES §4.7, ADR 0034, PLAN 19.6): an app lists the tools really installed,
//! including one its own build does not know, described by the contract saved with its package.
#![cfg(not(target_arch = "wasm32"))]

#[test]
fn an_installed_tool_unknown_to_this_build_is_listed_from_its_saved_contract() {
    let Some(target) = kortex_pkg::current_target() else {
        return;
    };
    let root = std::env::temp_dir().join(format!("kortex-shell-installed-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    let home = kortex_pkg::Home::at(root.join(".kortex"), &root);
    let registry: serde_json::Value =
        serde_json::from_str(include_str!("../../../registry/contracts.json")).expect("registry");
    let mut tools = std::collections::BTreeMap::new();
    for (id, contract) in [
        ("image.compress", {
            let mut c = registry["tools"][0].clone();
            c["tool"]["id"] = "image.compress".into();
            c["tool"]["version"] = "1.0.0".into();
            Some(c)
        }),
        ("text.cleaner", None),
        ("code.unknown-without-contract", None),
    ] {
        let version = if id == "image.compress" {
            "1.0.0"
        } else {
            "1.0.2"
        };
        let dir = home.tools_dir().join(id).join(version);
        std::fs::create_dir_all(&dir).expect("dir");
        std::fs::write(dir.join(kortex_pkg::binary_name(id, target)), b"bin").expect("binary");
        if let Some(c) = contract {
            std::fs::write(dir.join("contract.json"), c.to_string()).expect("contract");
        }
        tools.insert(
            id.to_owned(),
            kortex_pkg::InstalledTool {
                version: version.into(),
                installed_at: 0,
                binary: format!(
                    "tools/{id}/{version}/{}",
                    kortex_pkg::binary_name(id, target)
                ),
            },
        );
    }
    home.save_registry(&kortex_pkg::Registry { schema: 1, tools })
        .expect("registry");
    // Only test in this binary: no other thread reads the environment.
    unsafe { std::env::set_var("KORTEX_HOME", root.join(".kortex")) };
    let ids: Vec<String> = kortex_shell::available()
        .into_iter()
        .map(|e| e.tool.id)
        .collect();
    assert!(ids.contains(&"image.compress".to_owned()), "{ids:?}");
    assert!(ids.contains(&"text.cleaner".to_owned()), "{ids:?}");
    assert!(
        !ids.contains(&"code.unknown-without-contract".to_owned()),
        "{ids:?}"
    );
    assert_eq!(ids.len(), 2, "only installed tools: {ids:?}");
    let _ = std::fs::remove_dir_all(&root);
}
