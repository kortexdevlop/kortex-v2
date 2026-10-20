use std::fs;
use std::path::PathBuf;

use kortex_cli::pkgcmd::PkgEnv;
use kortex_pkg::{Home, InstalledTool, Registry};

/// Desktop binaries built by `cargo xtask build-desktop <id>` (skipped when absent): the CLI links
/// no tool, so these tests install the real binaries into a temporary home.
fn built(id: &str) -> Option<PathBuf> {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../dist/desktop");
    let target = kortex_pkg::current_target()?;
    let path = root
        .join(target)
        .join(id)
        .join(kortex_pkg::binary_name(id, target));
    path.is_file().then_some(path)
}

fn tmp(name: &str) -> PathBuf {
    let d = std::env::temp_dir().join(format!("kortex-cli-test-{}-{name}", std::process::id()));
    let _ = fs::remove_dir_all(&d);
    fs::create_dir_all(&d).expect("tmp dir");
    d
}

/// A home with the given tools installed from the local builds; `None` if one is not built.
fn installed(name: &str, ids: &[&str]) -> Option<PkgEnv> {
    let dir = tmp(name);
    let target = kortex_pkg::current_target()?;
    let home = Home::at(dir.join(".kortex"), &dir);
    let mut registry = Registry {
        schema: 1,
        tools: Default::default(),
    };
    for id in ids {
        let binary = built(id)?;
        let rel = format!("tools/{id}/1.0.0/{}", kortex_pkg::binary_name(id, target));
        let to = home.root().join(&rel);
        fs::create_dir_all(to.parent().expect("parent")).expect("dirs");
        fs::copy(binary, &to).expect("copy binary");
        registry.tools.insert(
            (*id).to_owned(),
            InstalledTool {
                version: "1.0.0".into(),
                installed_at: 0,
                binary: rel,
            },
        );
    }
    home.save_registry(&registry).expect("registry");
    Some(PkgEnv {
        channel: "stable".into(),
        home,
        index_src: dir
            .join("no-index/index.json")
            .to_string_lossy()
            .into_owned(),
        public_key: String::new(),
        target: Some(target.to_owned()),
        api: String::new(),
    })
}

fn run_in(env: &PkgEnv, args: &[&str]) -> Result<String, String> {
    let args: Vec<String> = args.iter().map(|s| (*s).to_owned()).collect();
    let mut out = Vec::new();
    kortex_cli::execute_with(&args, &mut out, Some(env))
        .map(|()| String::from_utf8(out).expect("utf8"))
}

#[test]
fn the_cli_links_no_tool_crate() {
    let manifest =
        fs::read_to_string(concat!(env!("CARGO_MANIFEST_DIR"), "/Cargo.toml")).expect("manifest");
    assert!(
        !manifest
            .lines()
            .any(|l| l.trim_start().starts_with("tool-")),
        "a tool crate is linked into the CLI: it would be rebuilt for every tool change"
    );
}

#[test]
fn a_tool_that_is_not_installed_says_how_to_install_it() {
    let dir = tmp("none");
    let env = PkgEnv {
        channel: "stable".into(),
        home: Home::at(dir.join(".kortex"), &dir),
        index_src: dir
            .join("no-index/index.json")
            .to_string_lossy()
            .into_owned(),
        public_key: String::new(),
        target: kortex_pkg::current_target().map(str::to_owned),
        api: String::new(),
    };
    let err = run_in(
        &env,
        &["run", "secure.hash-calculator", "--input", "text=a"],
    )
    .expect_err("not installed");
    assert!(
        err.contains("kortex install secure.hash-calculator"),
        "{err}"
    );
    assert!(run_in(&env, &["list"]).expect("empty list").is_empty());
}

#[test]
fn run_text_tool_and_typed_params() {
    let Some(env) = installed("text", &["secure.hash-calculator"]) else {
        return;
    };
    let out = run_in(
        &env,
        &[
            "run",
            "secure.hash-calculator",
            "--input",
            "text=abc",
            "--param",
            "algorithm=md5",
        ],
    )
    .expect("run");
    assert!(
        out.contains("hash: 900150983cd24fb0d6963f7d28e17f72"),
        "{out}"
    );
    let upper = run_in(
        &env,
        &[
            "run",
            "secure.hash-calculator",
            "--input",
            "text=abc",
            "--param",
            "algorithm=md5",
            "--param",
            "uppercase=true",
        ],
    )
    .expect("run");
    assert!(
        upper.contains("900150983CD24FB0D6963F7D28E17F72"),
        "{upper}"
    );
}

#[test]
fn file_inputs_via_at_path_and_text_from_file() {
    let Some(env) = installed("files", &["secure.hash-calculator"]) else {
        return;
    };
    let d = tmp("files-data");
    fs::write(d.join("a.txt"), "abc").expect("write");
    let by_file = run_in(
        &env,
        &[
            "run",
            "secure.hash-calculator",
            "--input",
            &format!("file=@{}", d.join("a.txt").display()),
            "--param",
            "algorithm=sha1",
        ],
    )
    .expect("run");
    assert!(
        by_file.contains("a9993e364706816aba3e25717850c26c9cd0d89d"),
        "{by_file}"
    );
    let by_text_file = run_in(
        &env,
        &[
            "run",
            "secure.hash-calculator",
            "--input",
            &format!("text=@{}", d.join("a.txt").display()),
        ],
    )
    .expect("run");
    assert!(
        by_text_file.contains("ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"),
        "{by_text_file}"
    );
}

#[test]
fn json_output_is_machine_readable() {
    let Some(env) = installed("json", &["secure.hash-calculator"]) else {
        return;
    };
    let out = run_in(
        &env,
        &[
            "run",
            "secure.hash-calculator",
            "--input",
            "text=a\"b",
            "--param",
            "algorithm=md5",
            "--json",
        ],
    )
    .expect("run");
    let line = out.trim();
    assert!(
        line.starts_with("{\"hash\":\"") && line.ends_with('}'),
        "{line}"
    );
    assert!(line.contains("\"verdict\":\"none\""));
}

#[test]
fn list_search_info_and_errors_come_from_the_installed_tools() {
    let Some(env) = installed("info", &["secure.hash-calculator", "text.cleaner"]) else {
        return;
    };
    let listed = run_in(&env, &["list", "--lang", "en"]).expect("list");
    assert!(
        listed
            .lines()
            .any(|l| l.starts_with("secure.hash-calculator\t")),
        "{listed}"
    );
    assert!(
        listed.lines().any(|l| l.starts_with("text.cleaner\t")),
        "{listed}"
    );
    assert_eq!(listed.lines().count(), 2, "only installed tools: {listed}");
    assert!(
        run_in(&env, &["search", "hash"])
            .expect("search")
            .contains("secure.hash-calculator")
    );
    assert!(
        run_in(&env, &["search", "zzzzqq"])
            .expect("search")
            .is_empty()
    );
    let info = run_in(&env, &["info", "secure.hash-calculator"]).expect("info");
    assert!(
        info.contains("param") && info.contains("algorithm"),
        "{info}"
    );
    assert!(run_in(&env, &["info", "nope.nope"]).is_err_and(|e| e.contains("outil inconnu")));
    assert!(
        run_in(
            &env,
            &["run", "secure.hash-calculator", "--param", "bogus=1"]
        )
        .is_err_and(|e| e.contains("paramètre inconnu"))
    );
    assert!(
        run_in(
            &env,
            &[
                "run",
                "secure.hash-calculator",
                "--input",
                "text=a",
                "--param",
                "uppercase=maybe"
            ]
        )
        .is_err_and(|e| e.contains("booléen"))
    );
    assert!(
        run_in(
            &env,
            &[
                "run",
                "secure.hash-calculator",
                "--input",
                "file=@/definitely/missing.bin"
            ]
        )
        .is_err_and(|e| e.contains("impossible"))
    );
    assert!(run_in(&env, &["frobnicate"]).is_err());
    assert!(run_in(&env, &[]).expect("usage").contains("Usage"));
}
