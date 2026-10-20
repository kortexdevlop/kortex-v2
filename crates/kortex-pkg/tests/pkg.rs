use std::collections::BTreeMap;
use std::io::{Read, Write};
use std::net::TcpListener;
use std::path::{Path, PathBuf};

use flate2::Compression;
use flate2::write::GzEncoder;
use kortex_pkg::{
    Artifact, CliRelease, Home, Index, Kind, Manifest, ToolRelease, binary_name, binary_name_cli,
    doctor, install, parse_verified, public_key_b64, remove, self_update, sha256_hex, shortcuts,
    sign_bytes, statuses, update,
};

const SEED: [u8; 32] = [7; 32];
const TARGET: &str = "x86_64-unknown-linux-gnu";

struct Env {
    dir: PathBuf,
    home: Home,
}

fn env(name: &str) -> Env {
    let dir = std::env::temp_dir().join(format!("kortex-pkg-{}-{name}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(dir.join("rel")).expect("dirs");
    let home = Home::at(dir.join("home/.kortex"), dir.join("home"));
    Env { dir, home }
}

impl Env {
    fn rel(&self) -> PathBuf {
        self.dir.join("rel")
    }
    fn index_path(&self) -> String {
        self.rel().join("index.json").to_string_lossy().into_owned()
    }
}

/// `entries`: (path inside the archive, bytes). Raw name bytes so tests can craft hostile paths.
fn tgz(
    entries: &[(&str, &[u8])],
    extra: impl FnOnce(&mut tar::Builder<GzEncoder<Vec<u8>>>),
) -> Vec<u8> {
    let mut b = tar::Builder::new(GzEncoder::new(Vec::new(), Compression::fast()));
    for (name, data) in entries {
        let mut h = tar::Header::new_gnu();
        h.set_size(data.len() as u64);
        h.set_mode(0o755);
        h.set_entry_type(tar::EntryType::Regular);
        h.as_old_mut().name[..name.len()].copy_from_slice(name.as_bytes());
        h.set_cksum();
        b.append(&h, *data).expect("append");
    }
    extra(&mut b);
    b.into_inner().expect("tar").finish().expect("gz")
}

fn manifest(id: &str, version: &str, target: &str, bin: &[u8]) -> Vec<u8> {
    serde_json::to_vec(&Manifest {
        id: id.into(),
        version: version.into(),
        target: target.into(),
        binary: binary_name(id, target),
        sha256_binary: sha256_hex(bin),
    })
    .expect("json")
}

fn package(id: &str, version: &str, bin: &[u8]) -> Vec<u8> {
    let m = manifest(id, version, TARGET, bin);
    tgz(
        &[("manifest.json", &m), (&binary_name(id, TARGET), bin)],
        |_| {},
    )
}

fn artifact(e: &Env, file: &str, bytes: &[u8]) -> Artifact {
    std::fs::write(e.rel().join(file), bytes).expect("write artifact");
    Artifact {
        url: file.to_owned(),
        sha256: sha256_hex(bytes),
        size: bytes.len() as u64,
    }
}

fn tool(version: &str, a: Artifact) -> ToolRelease {
    ToolRelease {
        name: BTreeMap::from([
            ("en".into(), "Hash Calculator".into()),
            ("fr".into(), "Calculateur de hash".into()),
        ]),
        universe: "secure".into(),
        version: version.into(),
        targets: BTreeMap::from([(TARGET.to_owned(), a)]),
    }
}

/// Writes index.json + index.json.sig, returns the verified index.
fn publish(e: &Env, tools: BTreeMap<String, ToolRelease>, cli: Option<CliRelease>) -> Index {
    let index = Index {
        schema: 1,
        generated: "2026-10-16".into(),
        cli,
        tools,
    };
    let bytes = serde_json::to_vec_pretty(&index).expect("json");
    std::fs::write(e.rel().join("index.json"), &bytes).expect("index");
    std::fs::write(e.rel().join("index.json.sig"), sign_bytes(&SEED, &bytes)).expect("sig");
    parse_verified(&bytes, &sign_bytes(&SEED, &bytes), &public_key_b64(&SEED)).expect("verified")
}

fn sink() -> impl FnMut(&str) {
    |_| {}
}

fn leftovers(home: &Home) -> Vec<String> {
    let mut out = Vec::new();
    if let Ok(rd) = std::fs::read_dir(home.tools_dir().join("secure.hash-calculator")) {
        out.extend(
            rd.filter_map(Result::ok)
                .map(|e| e.file_name().to_string_lossy().into_owned()),
        );
    }
    out
}

const ID: &str = "secure.hash-calculator";

#[test]
fn signature_must_match_the_exact_bytes_and_the_key() {
    let e = env("sig");
    publish(&e, BTreeMap::new(), None);
    let bytes = std::fs::read(e.rel().join("index.json")).expect("read");
    let sig = sign_bytes(&SEED, &bytes);
    let key = public_key_b64(&SEED);
    assert!(parse_verified(&bytes, &sig, &key).is_ok());
    let mut tampered = bytes.clone();
    tampered.extend_from_slice(b" ");
    assert_eq!(
        parse_verified(&tampered, &sig, &key)
            .expect_err("tampered")
            .kind(),
        Kind::BadSignature
    );
    assert_eq!(
        parse_verified(&bytes, &sig, &public_key_b64(&[8; 32]))
            .expect_err("wrong key")
            .kind(),
        Kind::BadSignature
    );
    assert_eq!(
        parse_verified(&bytes, "AAAA", &key)
            .expect_err("bad sig")
            .kind(),
        Kind::BadSignature
    );
    assert_eq!(
        parse_verified(&bytes, &sig, "garbage")
            .expect_err("bad key")
            .kind(),
        Kind::BadSignature
    );
    let v2 = br#"{"schema":2,"generated":"x","tools":{}}"#;
    assert_eq!(
        parse_verified(v2, &sign_bytes(&SEED, v2), &key)
            .expect_err("schema")
            .kind(),
        Kind::Unsupported
    );
}

#[test]
fn install_happy_path_registers_binary_manifest_and_launcher() {
    let e = env("happy");
    let a = artifact(
        &e,
        "p1.tar.gz",
        &package(ID, "1.0.0", b"#!/bin/sh\necho hi\n"),
    );
    let index = publish(&e, BTreeMap::from([(ID.into(), tool("1.0.0", a))]), None);
    let mut phases = Vec::new();
    let t = install(&e.home, &index, &e.index_path(), ID, TARGET, &mut |p| {
        phases.push(p.to_owned())
    })
    .expect("install");
    assert_eq!(phases, ["download", "verify", "install", "done"]);
    assert_eq!(t.version, "1.0.0");
    let bin = e.home.binary_path(&t);
    assert_eq!(
        std::fs::read(&bin).expect("binary"),
        b"#!/bin/sh\necho hi\n"
    );
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        assert_eq!(
            std::fs::metadata(&bin).expect("meta").permissions().mode() & 0o111,
            0o111,
            "must be executable"
        );
    }
    assert!(bin.parent().expect("dir").join("manifest.json").exists());
    assert_eq!(
        e.home.load_registry().expect("registry").tools[ID].version,
        "1.0.0"
    );
    assert!(
        !shortcuts::existing(&e.home, ID, "Hash Calculator").is_empty(),
        "launcher created"
    );
    assert_eq!(leftovers(&e.home), ["1.0.0"], "no staging leftovers");
}

#[test]
fn a_package_that_differs_from_the_signed_hash_is_refused_and_leaves_nothing() {
    let e = env("tamper");
    let a = artifact(&e, "p.tar.gz", &package(ID, "1.0.0", b"good"));
    let index = publish(&e, BTreeMap::from([(ID.into(), tool("1.0.0", a))]), None);
    std::fs::write(e.rel().join("p.tar.gz"), package(ID, "1.0.0", b"EVIL")).expect("swap");
    let err =
        install(&e.home, &index, &e.index_path(), ID, TARGET, &mut sink()).expect_err("refused");
    assert_eq!(err.kind(), Kind::Integrity);
    assert!(e.home.load_registry().expect("registry").tools.is_empty());
    assert!(!e.home.tools_dir().join(ID).exists());
}

#[test]
fn size_mismatch_is_refused() {
    let e = env("size");
    let mut a = artifact(&e, "p.tar.gz", &package(ID, "1.0.0", b"x"));
    a.size += 1;
    let index = publish(&e, BTreeMap::from([(ID.into(), tool("1.0.0", a))]), None);
    assert_eq!(
        install(&e.home, &index, &e.index_path(), ID, TARGET, &mut sink())
            .expect_err("size")
            .kind(),
        Kind::Integrity
    );
}

#[test]
fn hostile_archives_are_rejected_even_when_hash_and_signature_are_valid() {
    // A compromised build pipeline could sign a malicious package: extraction must still be safe.
    let bin = b"bin";
    let m = manifest(ID, "1.0.0", TARGET, bin);
    let name = binary_name(ID, TARGET);
    let cases: Vec<(&str, Vec<u8>)> = vec![
        (
            "path traversal",
            tgz(
                &[("manifest.json", &m), (&name, bin), ("../evil.txt", b"x")],
                |_| {},
            ),
        ),
        (
            "absolute path",
            tgz(
                &[("manifest.json", &m), (&name, bin), ("/tmp/evil.txt", b"x")],
                |_| {},
            ),
        ),
        (
            "extra file",
            tgz(
                &[("manifest.json", &m), (&name, bin), ("README", b"x")],
                |_| {},
            ),
        ),
        (
            "nested binary",
            tgz(
                &[("manifest.json", &m), (&format!("sub/{name}"), bin)],
                |_| {},
            ),
        ),
        (
            "symlink",
            tgz(&[("manifest.json", &m), (&name, bin)], |b| {
                let mut h = tar::Header::new_gnu();
                h.set_entry_type(tar::EntryType::Symlink);
                h.set_size(0);
                h.set_link_name("/etc/passwd").expect("link");
                h.set_path("link").expect("path");
                h.set_cksum();
                b.append(&h, std::io::empty()).expect("append");
            }),
        ),
        ("missing manifest", tgz(&[(&name, bin)], |_| {})),
        ("missing binary", tgz(&[("manifest.json", &m)], |_| {})),
        (
            "duplicate binary",
            tgz(&[("manifest.json", &m), (&name, bin), (&name, bin)], |_| {}),
        ),
        (
            "binary differs from manifest hash",
            tgz(&[("manifest.json", &m), (&name, b"other")], |_| {}),
        ),
        ("not gzip", b"definitely not an archive".to_vec()),
    ];
    for (label, archive) in cases {
        let e = env("hostile");
        let a = artifact(&e, "p.tar.gz", &archive);
        let index = publish(&e, BTreeMap::from([(ID.into(), tool("1.0.0", a))]), None);
        let err =
            install(&e.home, &index, &e.index_path(), ID, TARGET, &mut sink()).expect_err(label);
        assert!(
            matches!(err.kind(), Kind::Unsafe | Kind::Integrity),
            "{label}: {:?}",
            err.kind()
        );
        assert!(
            e.home.load_registry().expect("registry").tools.is_empty(),
            "{label}: registry must stay empty"
        );
        assert!(
            !e.dir.join("home/evil.txt").exists() && !e.home.root().join("evil.txt").exists(),
            "{label}"
        );
        assert!(
            !e.home.tools_dir().join(ID).join("1.0.0").exists(),
            "{label}: nothing installed"
        );
    }
}

#[test]
fn manifest_must_agree_with_the_signed_index() {
    for (label, m) in [
        ("other id", manifest("pdf.merge", "1.0.0", TARGET, b"b")),
        ("other version", manifest(ID, "9.9.9", TARGET, b"b")),
        (
            "other target",
            manifest(ID, "1.0.0", "aarch64-apple-darwin", b"b"),
        ),
    ] {
        let e = env("manifest");
        let archive = tgz(
            &[("manifest.json", &m), (&binary_name(ID, TARGET), b"b")],
            |_| {},
        );
        let index = publish(
            &e,
            BTreeMap::from([(ID.into(), tool("1.0.0", artifact(&e, "p.tar.gz", &archive)))]),
            None,
        );
        let r = install(&e.home, &index, &e.index_path(), ID, TARGET, &mut sink());
        assert!(r.is_err(), "{label}");
    }
}

#[test]
fn hostile_ids_unknown_tools_and_missing_targets_are_refused() {
    let e = env("ids");
    let index = publish(
        &e,
        BTreeMap::from([(
            ID.into(),
            tool(
                "1.0.0",
                artifact(&e, "p.tar.gz", &package(ID, "1.0.0", b"b")),
            ),
        )]),
        None,
    );
    for bad in ["../x.y", "a/b.c", "secure.hash-calculator/../.."] {
        assert_eq!(
            install(&e.home, &index, &e.index_path(), bad, TARGET, &mut sink())
                .expect_err(bad)
                .kind(),
            Kind::Unsafe,
            "{bad}"
        );
    }
    assert_eq!(
        install(
            &e.home,
            &index,
            &e.index_path(),
            "nope.nothing",
            TARGET,
            &mut sink()
        )
        .expect_err("unknown")
        .kind(),
        Kind::NotFound
    );
    assert_eq!(
        install(
            &e.home,
            &index,
            &e.index_path(),
            ID,
            "riscv64-unknown-none",
            &mut sink()
        )
        .expect_err("target")
        .kind(),
        Kind::Unsupported
    );
}

#[test]
fn update_installs_newer_versions_only_and_cleans_old_ones() {
    let e = env("update");
    let v1 = publish(
        &e,
        BTreeMap::from([(
            ID.into(),
            tool(
                "1.0.0",
                artifact(&e, "p1.tar.gz", &package(ID, "1.0.0", b"v1")),
            ),
        )]),
        None,
    );
    assert!(
        update(&e.home, &v1, &e.index_path(), ID, TARGET, &mut sink()).is_err(),
        "not installed yet"
    );
    install(&e.home, &v1, &e.index_path(), ID, TARGET, &mut sink()).expect("install");
    assert_eq!(
        update(&e.home, &v1, &e.index_path(), ID, TARGET, &mut sink()).expect("update"),
        None,
        "already current"
    );
    let reg = e.home.load_registry().expect("registry");
    assert!(statuses(&v1, &reg, Some(TARGET))[0].installed.as_deref() == Some("1.0.0"));
    let v2 = publish(
        &e,
        BTreeMap::from([(
            ID.into(),
            tool(
                "1.1.0",
                artifact(&e, "p2.tar.gz", &package(ID, "1.1.0", b"v2")),
            ),
        )]),
        None,
    );
    let st = &statuses(&v2, &reg, Some(TARGET))[0];
    assert!(st.update_available && st.supported);
    let t = update(&e.home, &v2, &e.index_path(), ID, TARGET, &mut sink())
        .expect("update")
        .expect("new");
    assert_eq!(std::fs::read(e.home.binary_path(&t)).expect("bin"), b"v2");
    assert_eq!(leftovers(&e.home), ["1.1.0"], "old version removed");
    // A failed update keeps the working version.
    std::fs::write(e.rel().join("p3.tar.gz"), b"junk").expect("junk");
    let v3 = publish(
        &e,
        BTreeMap::from([(
            ID.into(),
            tool(
                "1.2.0",
                Artifact {
                    url: "p3.tar.gz".into(),
                    sha256: sha256_hex(b"junk"),
                    size: 4,
                },
            ),
        )]),
        None,
    );
    assert!(update(&e.home, &v3, &e.index_path(), ID, TARGET, &mut sink()).is_err());
    assert_eq!(
        e.home.load_registry().expect("registry").tools[ID].version,
        "1.1.0"
    );
    assert_eq!(
        std::fs::read(e.home.binary_path(&t)).expect("bin"),
        b"v2",
        "still runnable"
    );
}

#[test]
fn remove_deletes_files_registry_entry_and_launcher() {
    let e = env("remove");
    let index = publish(
        &e,
        BTreeMap::from([(
            ID.into(),
            tool(
                "1.0.0",
                artifact(&e, "p.tar.gz", &package(ID, "1.0.0", b"b")),
            ),
        )]),
        None,
    );
    install(&e.home, &index, &e.index_path(), ID, TARGET, &mut sink()).expect("install");
    remove(&e.home, ID, "Hash Calculator").expect("remove");
    assert!(e.home.load_registry().expect("registry").tools.is_empty());
    assert!(!e.home.tools_dir().join(ID).exists());
    assert!(shortcuts::existing(&e.home, ID, "Hash Calculator").is_empty());
    assert_eq!(
        remove(&e.home, ID, "Hash Calculator")
            .expect_err("twice")
            .kind(),
        Kind::NotFound
    );
    assert!(remove(&e.home, "../x.y", "x").is_err());
}

#[test]
fn doctor_detects_tampering_and_missing_binaries() {
    let e = env("doctor");
    let index = publish(
        &e,
        BTreeMap::from([(
            ID.into(),
            tool(
                "1.0.0",
                artifact(&e, "p.tar.gz", &package(ID, "1.0.0", b"genuine")),
            ),
        )]),
        None,
    );
    let t = install(&e.home, &index, &e.index_path(), ID, TARGET, &mut sink()).expect("install");
    let find = |checks: &[kortex_pkg::Check]| {
        checks
            .iter()
            .find(|c| c.name == ID)
            .cloned()
            .expect("check")
    };
    assert!(find(&doctor(&e.home)).ok);
    std::fs::write(e.home.binary_path(&t), b"tampered").expect("tamper");
    let bad = find(&doctor(&e.home));
    assert!(!bad.ok && bad.detail.contains("modifié"), "{bad:?}");
    std::fs::remove_file(e.home.binary_path(&t)).expect("rm");
    assert!(!find(&doctor(&e.home)).ok);
    std::fs::write(e.home.registry_path(), b"{ not json").expect("corrupt");
    assert!(
        doctor(&e.home)
            .iter()
            .any(|c| c.name == "registre" && !c.ok)
    );
}

fn serve(
    files: BTreeMap<String, Vec<u8>>,
    requests: usize,
) -> (String, std::thread::JoinHandle<()>) {
    let listener = TcpListener::bind("127.0.0.1:0").expect("bind");
    let base = format!(
        "http://127.0.0.1:{}",
        listener.local_addr().expect("addr").port()
    );
    let h = std::thread::spawn(move || {
        for _ in 0..requests {
            let (mut s, _) = listener.accept().expect("accept");
            let mut buf = [0_u8; 4096];
            let n = s.read(&mut buf).expect("read");
            let req = String::from_utf8_lossy(&buf[..n]);
            let path = req.split_whitespace().nth(1).unwrap_or("/").to_owned();
            match files.get(&path) {
                Some(body) => {
                    let _ = write!(
                        s,
                        "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                        body.len()
                    );
                    let _ = s.write_all(body);
                }
                None => {
                    let _ = write!(
                        s,
                        "HTTP/1.1 404 Not Found\r\nContent-Length: 0\r\nConnection: close\r\n\r\n"
                    );
                }
            }
        }
    });
    (base, h)
}

#[test]
fn install_over_http_from_a_local_server() {
    let e = env("http");
    let pkg = package(ID, "1.0.0", b"over-http");
    let a = artifact(&e, "p.tar.gz", &pkg);
    publish(&e, BTreeMap::from([(ID.into(), tool("1.0.0", a))]), None);
    let files = BTreeMap::from([
        (
            "/r/index.json".to_owned(),
            std::fs::read(e.rel().join("index.json")).expect("i"),
        ),
        (
            "/r/index.json.sig".to_owned(),
            std::fs::read(e.rel().join("index.json.sig")).expect("s"),
        ),
        ("/r/p.tar.gz".to_owned(), pkg),
    ]);
    let (base, server) = serve(files, 3);
    let src = format!("{base}/r/index.json");
    let idx_bytes = kortex_pkg::fetch(&src, 1 << 20).expect("index");
    let sig = String::from_utf8(kortex_pkg::fetch(&format!("{src}.sig"), 1 << 10).expect("sig"))
        .expect("utf8");
    let index = parse_verified(&idx_bytes, &sig, &public_key_b64(&SEED)).expect("verified");
    let t = install(&e.home, &index, &src, ID, TARGET, &mut sink()).expect("install over http");
    assert_eq!(
        std::fs::read(e.home.binary_path(&t)).expect("bin"),
        b"over-http"
    );
    server.join().expect("server");
    // A 404 is a network error, never a panic.
    let (base2, s2) = serve(BTreeMap::new(), 1);
    assert_eq!(
        kortex_pkg::fetch(&format!("{base2}/missing"), 100)
            .expect_err("404")
            .kind(),
        Kind::Network
    );
    s2.join().expect("server");
}

fn cli_package(version: &str, bin: &[u8]) -> Vec<u8> {
    let name = binary_name_cli(TARGET);
    let m = serde_json::to_vec(&Manifest {
        id: "cli".into(),
        version: version.into(),
        target: TARGET.into(),
        binary: name.clone(),
        sha256_binary: sha256_hex(bin),
    })
    .expect("json");
    tgz(&[("manifest.json", &m), (&name, bin)], |_| {})
}

#[test]
fn self_update_swaps_the_binary_only_when_newer_and_verified() {
    let e = env("self");
    let exe: &Path = &e.dir.join("kortex");
    std::fs::write(exe, b"old-cli").expect("exe");
    let cli = |v: &str, bin: &[u8]| CliRelease {
        version: v.into(),
        targets: BTreeMap::from([(
            TARGET.to_owned(),
            artifact(&e, &format!("cli-{v}.tar.gz"), &cli_package(v, bin)),
        )]),
    };
    let index = publish(&e, BTreeMap::new(), Some(cli("0.2.0", b"new-cli")));
    assert_eq!(
        self_update(&index, &e.index_path(), exe, TARGET, "0.2.0").expect("same"),
        None,
        "already current"
    );
    assert_eq!(std::fs::read(exe).expect("exe"), b"old-cli");
    assert_eq!(
        self_update(&index, &e.index_path(), exe, TARGET, "0.1.0").expect("update"),
        Some("0.2.0".into())
    );
    assert_eq!(std::fs::read(exe).expect("exe"), b"new-cli");
    assert!(!exe.with_extension("old").exists() && !exe.with_extension("new").exists());
    // Tampered download: the current CLI must be untouched.
    std::fs::write(exe, b"current").expect("exe");
    let bad = publish(&e, BTreeMap::new(), Some(cli("0.3.0", b"genuine")));
    std::fs::write(
        e.rel().join("cli-0.3.0.tar.gz"),
        cli_package("0.3.0", b"EVIL"),
    )
    .expect("swap");
    assert!(self_update(&bad, &e.index_path(), exe, TARGET, "0.1.0").is_err());
    assert_eq!(std::fs::read(exe).expect("exe"), b"current");
}

#[test]
fn app_launchers_do_not_double_the_kortex_prefix_and_old_names_are_removed() {
    let e = env("launcher-names");
    let bin = e.home.root().join("bin-store");
    std::fs::create_dir_all(e.home.root()).expect("root");
    std::fs::write(&bin, b"#!/bin/sh\n").expect("bin");
    shortcuts::create(&e.home, "apps.store", "Kortex Store", &bin);
    let made = shortcuts::existing(&e.home, "apps.store", "Kortex Store");
    assert_eq!(made.len(), 1, "{made:?}");
    let file = made[0]
        .file_name()
        .expect("name")
        .to_string_lossy()
        .into_owned();
    assert!(!file.contains("Kortex Kortex"), "{file}");
    assert!(file.contains("Store"), "{file}");
    if cfg!(target_os = "macos") {
        // A launcher named by an earlier release is cleaned up too.
        let legacy = e
            .home
            .user_home()
            .join("Applications/Kortex Kortex Store.app");
        std::fs::create_dir_all(&legacy).expect("legacy");
        shortcuts::create(&e.home, "apps.store", "Kortex Store", &bin);
        assert!(!legacy.exists(), "reinstall replaces the old name");
        std::fs::create_dir_all(&legacy).expect("legacy");
        shortcuts::remove(&e.home, "apps.store", "Kortex Store");
        assert!(!legacy.exists() && !made[0].exists());
    }
}
