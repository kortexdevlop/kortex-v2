use std::collections::BTreeMap;
use std::path::PathBuf;

use flate2::Compression;
use flate2::write::GzEncoder;
use kortex_cli::pkgcmd::PkgEnv;
use kortex_pkg::{
    Artifact, Home, Index, Manifest, ToolRelease, binary_name, public_key_b64, sha256_hex,
    sign_bytes,
};

const SEED: [u8; 32] = [3; 32];
const TARGET: &str = "x86_64-unknown-linux-gnu";
const ID: &str = "secure.hash-calculator";

struct Fx {
    dir: PathBuf,
    env: PkgEnv,
}

fn fx(name: &str) -> Fx {
    let dir = std::env::temp_dir().join(format!("kortex-cli-pkg-{}-{name}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(dir.join("rel")).expect("dirs");
    let api = installation_account();
    let env = PkgEnv {
        home: Home::at(dir.join("home/.kortex"), dir.join("home")).with_secret_store(
            std::sync::Arc::new(kortex_platform::secrets::MemorySecretStore::default()),
        ),
        index_src: dir.join("rel/index.json").to_string_lossy().into_owned(),
        public_key: public_key_b64(&SEED),
        target: Some(TARGET.to_owned()),
        api: api.clone(),
    };
    env.home
        .save_credentials(&kortex_pkg::Credentials {
            api,
            email: "fixture@example.test".into(),
            device_id: "fixture-device".into(),
            token: "fixture-token".into(),
        })
        .expect("isolated account fixture");
    Fx { dir, env }
}

// Real local account verification with a public fixture; never accesses the OS keychain.
fn installation_account() -> String {
    use std::io::{BufRead, Read, Write};
    let listener = std::net::TcpListener::bind("127.0.0.1:0").expect("bind");
    let url = format!("http://{}", listener.local_addr().expect("address"));
    std::thread::spawn(move || {
        for connection in listener.incoming() {
            let Ok(mut stream) = connection else { break };
            let mut reader = std::io::BufReader::new(&mut stream);
            let mut headers = String::new();
            loop {
                let mut line = String::new();
                if reader.read_line(&mut line).unwrap_or(0) == 0 || line == "\r\n" {
                    break;
                }
                headers.push_str(&line);
            }
            let length = headers
                .lines()
                .find_map(|line| {
                    line.to_ascii_lowercase()
                        .strip_prefix("content-length:")
                        .and_then(|v| v.trim().parse::<usize>().ok())
                })
                .unwrap_or(0);
            let mut body = vec![0; length];
            let _ = reader.read_exact(&mut body);
            let allowed = headers
                .to_ascii_lowercase()
                .contains("authorization: bearer fixture-token");
            let (status, reply) = if allowed {
                (
                    200,
                    r#"{"ok":true,"email":"fixture@example.test","data":{"confirm_install":false}}"#,
                )
            } else {
                (401, r#"{"error":"unauthenticated"}"#)
            };
            let _ = write!(
                stream,
                "HTTP/1.1 {status} OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{reply}",
                reply.len()
            );
        }
    });
    url
}

fn package(id: &str, version: &str, bin: &[u8]) -> Vec<u8> {
    let name = binary_name(id, TARGET);
    let m = serde_json::to_vec(&Manifest {
        id: id.into(),
        version: version.into(),
        target: TARGET.into(),
        binary: name.clone(),
        sha256_binary: sha256_hex(bin),
    })
    .expect("json");
    let mut b = tar::Builder::new(GzEncoder::new(Vec::new(), Compression::fast()));
    for (n, d) in [("manifest.json", m.as_slice()), (name.as_str(), bin)] {
        let mut h = tar::Header::new_gnu();
        h.set_size(d.len() as u64);
        h.set_mode(0o755);
        h.set_cksum();
        b.append_data(&mut h, n, d).expect("append");
    }
    b.into_inner().expect("tar").finish().expect("gz")
}

fn publish(f: &Fx, version: &str, bin: &[u8]) {
    let bytes = package(ID, version, bin);
    let file = format!("p-{version}.tar.gz");
    std::fs::write(f.dir.join("rel").join(&file), &bytes).expect("pkg");
    let tool = ToolRelease {
        name: BTreeMap::from([
            ("en".into(), "Hash Calculator".into()),
            ("fr".into(), "Calculateur de hash".into()),
        ]),
        universe: "secure".into(),
        version: version.into(),
        targets: BTreeMap::from([(
            TARGET.to_owned(),
            Artifact {
                url: file,
                sha256: sha256_hex(&bytes),
                size: bytes.len() as u64,
            },
        )]),
    };
    let index = Index {
        schema: 1,
        generated: "2026-10-16".into(),
        cli: None,
        tools: BTreeMap::from([(ID.into(), tool)]),
    };
    let json = serde_json::to_vec_pretty(&index).expect("json");
    std::fs::write(f.dir.join("rel/index.json"), &json).expect("index");
    std::fs::write(f.dir.join("rel/index.json.sig"), sign_bytes(&SEED, &json)).expect("sig");
}

fn run(f: &Fx, args: &[&str]) -> Result<String, String> {
    let args: Vec<String> = args.iter().map(|s| (*s).to_owned()).collect();
    let mut out = Vec::new();
    kortex_cli::execute_with(&args, &mut out, Some(&f.env))
        .map(|()| String::from_utf8(out).expect("utf8"))
}

#[test]
fn full_lifecycle_available_install_update_remove() {
    let f = fx("life");
    publish(&f, "1.0.0", b"#!/bin/sh\nexit 0\n");
    assert!(
        run(&f, &["available", "--lang", "fr"])
            .expect("available")
            .contains("Calculateur de hash")
            && run(&f, &["available"])
                .expect("available")
                .contains("disponible 1.0.0")
    );
    assert!(
        run(&f, &["installed"])
            .expect("installed")
            .contains("Aucun outil installé")
    );
    let out = run(&f, &["install", ID]).expect("install");
    assert!(
        out.contains("vérification") && out.contains("✓ secure.hash-calculator 1.0.0 installé"),
        "{out}"
    );
    assert!(
        run(&f, &["installed"])
            .expect("installed")
            .contains("secure.hash-calculator\t1.0.0")
    );
    assert!(
        run(&f, &["available"])
            .expect("available")
            .contains("installé 1.0.0")
    );
    assert!(
        run(&f, &["update", "--all"])
            .expect("update")
            .contains("est à jour")
    );
    publish(&f, "1.1.0", b"#!/bin/sh\nexit 0\n# v2\n");
    assert!(
        run(&f, &["installed"])
            .expect("installed")
            .contains("mise à jour disponible : 1.1.0")
    );
    assert!(
        run(&f, &["update", ID])
            .expect("update")
            .contains("mis à jour en 1.1.0")
    );
    assert!(
        run(&f, &["installed"])
            .expect("installed")
            .contains("1.1.0")
            && !run(&f, &["installed"])
                .expect("installed")
                .contains("mise à jour disponible")
    );
    assert!(
        run(&f, &["remove", ID])
            .expect("remove")
            .contains("supprimé")
    );
    assert!(
        run(&f, &["installed"])
            .expect("installed")
            .contains("Aucun outil installé")
    );
    assert!(run(&f, &["remove", ID]).is_err());
}

#[test]
fn doctor_reports_health_and_fails_on_problems() {
    let f = fx("doctor");
    publish(&f, "1.0.0", b"#!/bin/sh\nexit 0\n");
    run(&f, &["install", ID]).expect("install");
    let ok = run(&f, &["doctor"]).expect("doctor");
    assert!(
        ok.contains("✓ index — signature valide") && ok.contains("✓ secure.hash-calculator"),
        "{ok}"
    );
    let bin = std::fs::read_dir(f.dir.join("home/.kortex/tools").join(ID).join("1.0.0"))
        .expect("dir")
        .filter_map(Result::ok)
        .map(|e| e.path())
        .find(|p| p.extension().is_none() && p.file_name().is_some_and(|n| n != "manifest.json"))
        .expect("binary");
    std::fs::write(&bin, b"tampered").expect("tamper");
    let err = run(&f, &["doctor"]).expect_err("must fail");
    assert!(err.contains("problème"), "{err}");
}

#[cfg(unix)]
#[test]
fn open_launches_the_installed_binary() {
    let f = fx("open");
    let marker = f.dir.join("launched");
    publish(
        &f,
        "1.0.0",
        format!("#!/bin/sh\ntouch '{}'\n", marker.display()).as_bytes(),
    );
    run(&f, &["install", ID]).expect("install");
    assert!(run(&f, &["open", ID]).expect("open").contains("lancé"));
    for _ in 0..400 {
        if marker.exists() {
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(50));
    }
    assert!(marker.exists(), "the installed binary must actually run");
    assert!(
        run(&f, &["open", "pdf.merge"])
            .expect_err("not installed")
            .contains("n'est pas installé")
    );
}

#[test]
fn an_index_not_signed_by_our_key_is_refused_before_anything_is_downloaded() {
    let f = fx("badkey");
    publish(&f, "1.0.0", b"x");
    let mut env = fx("badkey2").env;
    env.index_src = f.env.index_src.clone();
    env.public_key = public_key_b64(&[4; 32]); // a different key
    let args = vec!["install".to_owned(), ID.to_owned()];
    let err = kortex_cli::execute_with(&args, &mut Vec::new(), Some(&env)).expect_err("refused");
    assert!(err.contains("signature"), "{err}");
    assert!(env.home.load_registry().expect("registry").tools.is_empty());
}

#[test]
fn a_modified_index_is_refused() {
    let f = fx("modified");
    publish(&f, "1.0.0", b"x");
    let p = f.dir.join("rel/index.json");
    let text = std::fs::read_to_string(&p)
        .expect("read")
        .replace("1.0.0", "9.9.9");
    std::fs::write(&p, text).expect("write");
    assert!(
        run(&f, &["install", ID])
            .expect_err("refused")
            .contains("signature")
    );
    assert!(run(&f, &["available"]).is_err());
}

#[test]
fn errors_are_clear_for_unknown_and_malicious_ids() {
    let f = fx("errors");
    publish(&f, "1.0.0", b"x");
    assert!(
        run(&f, &["install", "nope.nothing"])
            .expect_err("unknown")
            .contains("introuvable")
    );
    assert!(run(&f, &["install", "../../etc.passwd"]).is_err());
    assert!(run(&f, &["install"]).expect_err("usage").contains("usage"));
    assert!(
        run(&f, &["update", "--all"])
            .expect("none installed")
            .contains("Aucun outil installé")
    );
    let mut unsupported = fx("platform").env;
    unsupported.target = None;
    unsupported.index_src = f.env.index_src.clone();
    let args = vec!["install".to_owned(), ID.to_owned()];
    assert!(
        kortex_cli::execute_with(&args, &mut Vec::new(), Some(&unsupported))
            .expect_err("platform")
            .contains("plateforme")
    );
}

#[test]
fn engine_commands_still_work_without_any_package_environment() {
    let mut out = Vec::new();
    kortex_cli::execute_with(
        &[
            "run".to_owned(),
            ID.to_owned(),
            "--input".to_owned(),
            "text=abc".to_owned(),
            "--param".to_owned(),
            "algorithm=md5".to_owned(),
        ],
        &mut out,
        None,
    )
    .expect("run");
    assert!(
        String::from_utf8(out)
            .expect("utf8")
            .contains("900150983cd24fb0d6963f7d28e17f72")
    );
    assert!(
        kortex_cli::execute_with(
            &["install".to_owned(), ID.to_owned()],
            &mut Vec::new(),
            None
        )
        .is_err()
    );
}
