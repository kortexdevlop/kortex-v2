//! Installs the REAL built desktop apps through the real `kortex install` command: package, signed
//! index (throwaway key), signature and hash checks, registry, then a launch. Apps whose binary is
//! not built for this machine are skipped, so a plain `cargo test` stays hermetic; build them with
//! `cargo xtask build-app apps.<name>` first.
use std::collections::BTreeMap;
use std::io::{BufRead, Read, Write};
use std::path::PathBuf;
use std::time::Duration;

use flate2::Compression;
use flate2::write::GzEncoder;
use kortex_cli::pkgcmd::PkgEnv;
use kortex_pkg::{
    Artifact, Home, Index, Manifest, ToolRelease, binary_name, current_target, public_key_b64,
    sha256_hex, sign_bytes,
};

const SEED: [u8; 32] = [7; 32];

/// A local account answering the install confirmation check with a public fixture token.
fn account() -> String {
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
                .find_map(|l| {
                    l.to_ascii_lowercase()
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

fn package(id: &str, version: &str, target: &str, bin: &[u8]) -> Vec<u8> {
    let name = binary_name(id, target);
    let manifest = serde_json::to_vec(&Manifest {
        id: id.into(),
        version: version.into(),
        target: target.into(),
        binary: name.clone(),
        sha256_binary: sha256_hex(bin),
    })
    .expect("manifest");
    let mut tar = tar::Builder::new(GzEncoder::new(Vec::new(), Compression::fast()));
    for (n, d, mode) in [
        ("manifest.json", manifest.as_slice(), 0o644),
        (name.as_str(), bin, 0o755),
    ] {
        let mut h = tar::Header::new_gnu();
        h.set_size(d.len() as u64);
        h.set_mode(mode);
        h.set_cksum();
        tar.append_data(&mut h, n, d).expect("append");
    }
    tar.into_inner().expect("tar").finish().expect("gz")
}

#[test]
fn built_apps_install_through_the_signed_index_and_launch() {
    let Some(target) = current_target() else {
        eprintln!("skipped: unsupported host");
        return;
    };
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
    let registry: serde_json::Value = serde_json::from_str(
        &std::fs::read_to_string(root.join("registry/apps.json")).expect("registry"),
    )
    .expect("json");
    let dir = std::env::temp_dir().join(format!("kortex-install-apps-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(dir.join("rel")).expect("dirs");
    let mut tools = BTreeMap::new();
    for entry in registry["apps"].as_array().expect("apps") {
        let app = &entry["app"];
        if !matches!(app["status"].as_str(), Some("beta" | "stable")) {
            continue;
        }
        let id = app["id"].as_str().expect("id");
        let version = app["version"].as_str().expect("version");
        let built = root
            .join("dist/desktop")
            .join(target)
            .join(id)
            .join(binary_name(id, target));
        let Ok(bin) = std::fs::read(&built) else {
            eprintln!("skipped {id}: not built for {target}");
            continue;
        };
        let bytes = package(id, version, target, &bin);
        let file = format!("{id}-{version}.tar.gz");
        std::fs::write(dir.join("rel").join(&file), &bytes).expect("package");
        let names: BTreeMap<String, String> = app["name"]
            .as_object()
            .expect("name")
            .iter()
            .map(|(k, v)| (k.clone(), v.as_str().unwrap_or_default().to_owned()))
            .collect();
        tools.insert(
            id.to_owned(),
            (
                ToolRelease {
                    name: names,
                    universe: "apps".into(),
                    version: version.into(),
                    targets: BTreeMap::from([(
                        target.to_owned(),
                        Artifact {
                            url: file,
                            sha256: sha256_hex(&bytes),
                            size: bytes.len() as u64,
                        },
                    )]),
                },
                bin,
            ),
        );
    }
    if tools.is_empty() {
        eprintln!("skipped: no app built for this machine");
        return;
    }
    let index = Index {
        schema: 1,
        generated: "2026-10-18".into(),
        cli: None,
        tools: tools
            .iter()
            .map(|(k, (r, _))| (k.clone(), r.clone()))
            .collect(),
    };
    let json = serde_json::to_vec_pretty(&index).expect("index");
    std::fs::write(dir.join("rel/index.json"), &json).expect("index");
    std::fs::write(dir.join("rel/index.json.sig"), sign_bytes(&SEED, &json)).expect("signature");

    let api = account();
    let env = PkgEnv {
        home: Home::at(dir.join("home/.kortex"), dir.join("home")).with_secret_store(
            std::sync::Arc::new(kortex_platform::secrets::MemorySecretStore::default()),
        ),
        index_src: dir.join("rel/index.json").to_string_lossy().into_owned(),
        public_key: public_key_b64(&SEED),
        target: Some(target.to_owned()),
        api: api.clone(),
    };
    env.home
        .save_credentials(&kortex_pkg::Credentials {
            api,
            email: "fixture@example.test".into(),
            device_id: "fixture-device".into(),
            token: "fixture-token".into(),
        })
        .expect("fixture account");
    let run = |args: &[&str]| {
        let args: Vec<String> = args.iter().map(|s| (*s).to_owned()).collect();
        let mut out = Vec::new();
        kortex_cli::execute_with(&args, &mut out, Some(&env))
            .map(|()| String::from_utf8(out).expect("utf8"))
    };

    let available = run(&["available"]).expect("available");
    for id in tools.keys() {
        assert!(
            available.contains(id.as_str()),
            "{id} is offered by the signed index: {available}"
        );
    }
    for (id, (release, built)) in &tools {
        let out = run(&["install", id]).unwrap_or_else(|e| unreachable!("install {id}: {e}"));
        assert!(
            out.contains(&format!("✓ {id} {} installé", release.version)),
            "{out}"
        );
        let installed = kortex_pkg::installed_binary(&env.home, id)
            .unwrap_or_else(|| unreachable!("{id} is registered"));
        assert_eq!(
            std::fs::read(&installed).expect("installed binary"),
            *built,
            "{id}: the installed binary is the built one, byte for byte"
        );
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            assert_ne!(
                std::fs::metadata(&installed)
                    .expect("metadata")
                    .permissions()
                    .mode()
                    & 0o111,
                0,
                "{id} is executable"
            );
        }
        // The installed app really starts and stays up (a window is created; killed after 4 s).
        let mut child = std::process::Command::new(&installed)
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .spawn()
            .unwrap_or_else(|e| unreachable!("launch {id}: {e}"));
        std::thread::sleep(Duration::from_secs(4));
        assert!(
            child.try_wait().expect("status").is_none(),
            "{id} is still running after 4 s"
        );
        let _ = child.kill();
        let _ = child.wait();
    }
    let installed = run(&["installed"]).expect("installed");
    for id in tools.keys() {
        assert!(
            installed.contains(id.as_str()),
            "{id} listed as installed: {installed}"
        );
    }
    // Removal leaves nothing behind.
    for id in tools.keys() {
        assert!(run(&["remove", id]).is_ok());
        assert!(
            kortex_pkg::installed_binary(&env.home, id).is_none(),
            "{id} removed"
        );
    }
    let _ = std::fs::remove_dir_all(&dir);
}
