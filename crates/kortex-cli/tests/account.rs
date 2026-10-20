use std::collections::BTreeMap;
use std::io::{Read, Write};
use std::net::TcpListener;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use flate2::Compression;
use flate2::write::GzEncoder;
use kortex_cli::pkgcmd::{CliIo, PkgEnv};
use kortex_pkg::{
    Artifact, Home, Index, Manifest, ToolRelease, binary_name, derive_auth_key, public_key_b64,
    sha256_hex, sign_bytes,
};

const SEED: [u8; 32] = [5; 32];
const TARGET: &str = "x86_64-unknown-linux-gnu";
const ID: &str = "secure.hash-calculator";
const ITER: u32 = 1000;

#[derive(Debug, Clone)]
struct Req {
    method: String,
    path: String,
    auth: Option<String>,
    csrf: Option<String>,
    body: String,
}

type Handler = Box<dyn Fn(&Req) -> (u16, String) + Send>;

/// Tiny API stand-in that records every request it receives.
fn mock_api(handler: Handler) -> (String, Arc<Mutex<Vec<Req>>>) {
    let listener = TcpListener::bind("127.0.0.1:0").expect("bind");
    let base = format!(
        "http://127.0.0.1:{}",
        listener.local_addr().expect("addr").port()
    );
    let log = Arc::new(Mutex::new(Vec::new()));
    let log2 = Arc::clone(&log);
    std::thread::spawn(move || {
        for stream in listener.incoming() {
            let Ok(mut s) = stream else { break };
            let mut buf = Vec::new();
            let mut chunk = [0_u8; 4096];
            let (head_end, len) = loop {
                let n = s.read(&mut chunk).unwrap_or(0);
                if n == 0 {
                    break (0, 0);
                }
                buf.extend_from_slice(&chunk[..n]);
                if let Some(i) = buf.windows(4).position(|w| w == b"\r\n\r\n") {
                    let head = String::from_utf8_lossy(&buf[..i]).to_lowercase();
                    let len = head
                        .lines()
                        .find_map(|l| l.strip_prefix("content-length:"))
                        .and_then(|v| v.trim().parse::<usize>().ok())
                        .unwrap_or(0);
                    break (i + 4, len);
                }
            };
            if head_end == 0 {
                continue;
            }
            while buf.len() < head_end + len {
                let n = s.read(&mut chunk).unwrap_or(0);
                if n == 0 {
                    break;
                }
                buf.extend_from_slice(&chunk[..n]);
            }
            let head = String::from_utf8_lossy(&buf[..head_end]).into_owned();
            let mut lines = head.lines();
            let first: Vec<&str> = lines.next().unwrap_or("").split_whitespace().collect();
            let header = |name: &str| {
                head.lines().find_map(|l| {
                    l.to_lowercase()
                        .starts_with(&format!("{name}:"))
                        .then(|| l.split_once(':').map(|x| x.1.trim().to_owned()))
                        .flatten()
                })
            };
            let req = Req {
                method: first.first().copied().unwrap_or("").to_owned(),
                path: first.get(1).copied().unwrap_or("").to_owned(),
                auth: header("authorization"),
                csrf: header("x-kortex-request"),
                body: String::from_utf8_lossy(&buf[head_end..head_end + len]).into_owned(),
            };
            let (status, body) = handler(&req);
            log2.lock().expect("log").push(req);
            let _ = write!(
                s,
                "HTTP/1.1 {status} X\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
                body.len()
            );
        }
    });
    (base, log)
}

struct Fx {
    dir: PathBuf,
    env: PkgEnv,
}

fn fx(name: &str, api: &str) -> Fx {
    let dir = std::env::temp_dir().join(format!("kortex-cli-acct-{}-{name}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(dir.join("rel")).expect("dirs");
    // one tiny installable tool
    let bin = b"#!/bin/sh\nexit 0\n";
    let bn = binary_name(ID, TARGET);
    let m = serde_json::to_vec(&Manifest {
        id: ID.into(),
        version: "1.0.0".into(),
        target: TARGET.into(),
        binary: bn.clone(),
        sha256_binary: sha256_hex(bin),
    })
    .expect("json");
    let mut b = tar::Builder::new(GzEncoder::new(Vec::new(), Compression::fast()));
    for (n, d) in [
        ("manifest.json", m.as_slice()),
        (bn.as_str(), bin.as_slice()),
    ] {
        let mut h = tar::Header::new_gnu();
        h.set_size(d.len() as u64);
        h.set_mode(0o755);
        h.set_cksum();
        b.append_data(&mut h, n, d).expect("append");
    }
    let pkg = b.into_inner().expect("tar").finish().expect("gz");
    std::fs::write(dir.join("rel/p.tar.gz"), &pkg).expect("pkg");
    let tool = ToolRelease {
        name: BTreeMap::from([("en".into(), "Hash".into())]),
        universe: "secure".into(),
        version: "1.0.0".into(),
        targets: BTreeMap::from([(
            TARGET.to_owned(),
            Artifact {
                url: "p.tar.gz".into(),
                sha256: sha256_hex(&pkg),
                size: pkg.len() as u64,
            },
        )]),
    };
    let index = serde_json::to_vec_pretty(&Index {
        schema: 1,
        channel: "stable".into(),
        sequence: 0,
        source_commit: None,
        generated: "x".into(),
        cli: None,
        tools: BTreeMap::from([(ID.into(), tool)]),
    })
    .expect("json");
    std::fs::write(dir.join("rel/index.json"), &index).expect("index");
    std::fs::write(dir.join("rel/index.json.sig"), sign_bytes(&SEED, &index)).expect("sig");
    let env = PkgEnv {
        channel: "stable".into(),
        home: Home::at(dir.join("home/.kortex"), dir.join("home")).with_secret_store(
            std::sync::Arc::new(kortex_platform::secrets::MemorySecretStore::default()),
        ),
        index_src: dir.join("rel/index.json").to_string_lossy().into_owned(),
        public_key: public_key_b64(&SEED),
        target: Some(TARGET.to_owned()),
        api: api.to_owned(),
    };
    Fx { dir, env }
}

/// Scripted terminal: answers prompts from queues and records what was asked.
#[derive(Default)]
struct Script {
    emails: Vec<String>,
    passwords: Vec<String>,
    asked: Vec<String>,
}

impl CliIo for Script {
    fn prompt(&mut self, label: &str) -> Option<String> {
        self.asked.push(label.to_owned());
        (!self.emails.is_empty()).then(|| self.emails.remove(0))
    }
    fn password(&mut self, label: &str, _from_stdin: bool) -> Option<String> {
        self.asked.push(label.to_owned());
        (!self.passwords.is_empty()).then(|| self.passwords.remove(0))
    }
    fn kdf_iterations(&self) -> u32 {
        ITER
    }
}

fn run(f: &Fx, io: &mut Script, args: &[&str]) -> Result<String, String> {
    let args: Vec<String> = args.iter().map(|s| (*s).to_owned()).collect();
    let mut out = Vec::new();
    kortex_cli::execute_io(&args, &mut out, Some(&f.env), io)
        .map(|()| String::from_utf8(out).expect("utf8"))
}

fn api_ok() -> Handler {
    Box::new(|r| match (r.method.as_str(), r.path.as_str()) {
        ("POST", "/v1/auth/device") => (
            201,
            r#"{"device_id":"dev-1","token":"kdt_secret_token","email":"a@example.com"}"#.into(),
        ),
        ("GET", "/v1/me") => (200, r#"{"email":"a@example.com"}"#.into()),
        ("GET", "/v1/settings") => (200, r#"{"data":{},"updated_at":0}"#.into()),
        ("POST", "/v1/auth/confirm") => {
            let key = serde_json::from_str::<serde_json::Value>(&r.body)
                .ok()
                .and_then(|v| v["auth_key"].as_str().map(str::to_owned))
                .unwrap_or_default();
            if key == derive_auth_key("a@example.com", "correct horse battery", ITER) {
                (200, r#"{"ok":true}"#.into())
            } else {
                (403, r#"{"error":"invalid_credentials"}"#.into())
            }
        }
        ("POST", "/v1/installs") | ("DELETE", "/v1/devices/self") => (200, r#"{"ok":true}"#.into()),
        (m, p) if m == "DELETE" && p.starts_with("/v1/installs/") => (200, r#"{"ok":true}"#.into()),
        _ => (404, r#"{"error":"not_found"}"#.into()),
    })
}

fn login(f: &Fx) {
    let mut io = Script {
        emails: vec!["a@example.com".into()],
        passwords: vec!["correct horse battery".into()],
        ..Script::default()
    };
    run(f, &mut io, &["login", "--name", "Test Mac"]).expect("login");
}

#[test]
fn login_sends_a_derived_key_never_the_password_and_stores_a_private_token() {
    let (api, log) = mock_api(api_ok());
    let f = fx("login", &api);
    let mut io = Script {
        emails: vec!["A@Example.com ".into()],
        passwords: vec!["correct horse battery".into()],
        ..Script::default()
    };
    let out = run(&f, &mut io, &["login", "--name", "Test Mac"]).expect("login");
    assert!(out.contains("connecté en tant que a@example.com"), "{out}");
    let req = log.lock().expect("log")[0].clone();
    assert_eq!(
        (req.method.as_str(), req.path.as_str()),
        ("POST", "/v1/auth/device")
    );
    assert_eq!(req.csrf.as_deref(), Some("1"), "CSRF proof header");
    assert!(
        !req.body.contains("correct horse battery"),
        "the password must never be sent: {}",
        req.body
    );
    let body: serde_json::Value = serde_json::from_str(&req.body).expect("json");
    assert_eq!(
        body["auth_key"],
        derive_auth_key("a@example.com", "correct horse battery", ITER)
    );
    assert_eq!(
        (body["kdf"].as_u64(), body["name"].as_str()),
        (Some(1), Some("Test Mac"))
    );
    let c = f
        .env
        .home
        .load_credentials()
        .expect("store")
        .expect("credentials");
    assert_eq!(
        (c.token.as_str(), c.device_id.as_str(), c.email.as_str()),
        ("kdt_secret_token", "dev-1", "a@example.com")
    );
    assert!(
        !f.env.home.credentials_path().exists(),
        "no plaintext credential file"
    );

    assert!(
        !out.contains("kdt_secret_token"),
        "the token is never printed"
    );
}

#[test]
fn login_failure_stores_nothing_and_says_why() {
    let (api, _log) = mock_api(Box::new(|_| {
        (401, r#"{"error":"invalid_credentials"}"#.into())
    }));
    let f = fx("badlogin", &api);
    let mut io = Script {
        emails: vec!["a@example.com".into()],
        passwords: vec!["nope nope nope".into()],
        ..Script::default()
    };
    let err = run(&f, &mut io, &["login"]).expect_err("refused");
    assert!(err.contains("incorrect"), "{err}");
    assert!(f.env.home.load_credentials().expect("store").is_none());
    let mut none = Script::default();
    assert!(
        run(&f, &mut none, &["login"])
            .expect_err("no terminal")
            .contains("e-mail requis")
    );
}

#[test]
fn whoami_and_logout_use_the_bearer_token_and_clear_local_state() {
    let (api, log) = mock_api(api_ok());
    let f = fx("whoami", &api);
    let mut io = Script::default();
    assert!(
        run(&f, &mut io, &["whoami"])
            .expect("whoami")
            .contains("Non connecté")
    );
    login(&f);
    assert!(
        run(&f, &mut io, &["whoami"])
            .expect("whoami")
            .contains("a@example.com")
    );
    assert!(
        run(&f, &mut io, &["logout"])
            .expect("logout")
            .contains("révoqué")
    );
    assert!(f.env.home.load_credentials().expect("store").is_none());
    let reqs = log.lock().expect("log").clone();
    assert!(reqs.iter().any(|r| r.method == "GET"
        && r.path == "/v1/me"
        && r.auth.as_deref() == Some("Bearer kdt_secret_token")));
    assert!(reqs.iter().any(|r| r.method == "DELETE"
        && r.path == "/v1/devices/self"
        && r.auth.as_deref() == Some("Bearer kdt_secret_token")));
}

#[test]
fn install_when_logged_in_asks_for_the_password_and_records_the_install() {
    let (api, log) = mock_api(api_ok());
    let f = fx("install", &api);
    login(&f);
    let mut io = Script {
        passwords: vec!["correct horse battery".into()],
        ..Script::default()
    };
    let out = run(&f, &mut io, &["install", ID]).expect("install");
    assert!(
        out.contains("identité confirmée") && out.contains("installé"),
        "{out}"
    );
    assert!(
        io.asked
            .iter()
            .any(|a| a.contains("confirme l'installation"))
    );
    let reqs = log.lock().expect("log").clone();
    let rec = reqs
        .iter()
        .find(|r| r.path == "/v1/installs")
        .expect("install recorded");
    assert_eq!(rec.auth.as_deref(), Some("Bearer kdt_secret_token"));
    let body: serde_json::Value = serde_json::from_str(&rec.body).expect("json");
    assert_eq!(
        (
            body["tool"].as_str(),
            body["version"].as_str(),
            body["target"].as_str()
        ),
        (Some(ID), Some("1.0.0"), Some(TARGET))
    );
    assert!(
        !reqs
            .iter()
            .any(|r| r.body.contains("correct horse battery")),
        "password never on the wire"
    );
}

#[test]
fn a_wrong_password_blocks_the_installation_before_anything_is_downloaded() {
    let (api, log) = mock_api(api_ok());
    let f = fx("wrongpw", &api);
    login(&f);
    let mut io = Script {
        passwords: vec!["definitely wrong!!".into()],
        ..Script::default()
    };
    let err = run(&f, &mut io, &["install", ID]).expect_err("blocked");
    assert!(err.contains("mot de passe incorrect"), "{err}");
    assert!(
        f.env
            .home
            .load_registry()
            .expect("registry")
            .tools
            .is_empty()
    );
    assert!(!f.env.home.tools_dir().join(ID).exists());
    assert!(
        !log.lock()
            .expect("log")
            .iter()
            .any(|r| r.path == "/v1/installs")
    );
    let mut silent = Script::default(); // no terminal and no --password-stdin
    assert!(
        run(&f, &mut silent, &["install", ID])
            .expect_err("needs a password")
            .contains("mot de passe requis")
    );
}

#[test]
fn the_confirmation_can_be_disabled_in_the_account() {
    let (api, _log) = mock_api(Box::new(|r| match (r.method.as_str(), r.path.as_str()) {
        ("POST", "/v1/auth/device") => (
            201,
            r#"{"device_id":"d","token":"kdt_t","email":"a@example.com"}"#.into(),
        ),
        ("GET", "/v1/settings") => (200, r#"{"data":{"confirm_install":false}}"#.into()),
        ("POST", "/v1/installs") => (200, r#"{"ok":true}"#.into()),
        _ => (404, "{}".into()),
    }));
    let f = fx("noconfirm", &api);
    login(&f);
    let mut io = Script::default();
    let out = run(&f, &mut io, &["install", ID]).expect("install without prompt");
    assert!(!out.contains("identité confirmée") && out.contains("installé"));
    assert!(io.asked.is_empty(), "no password prompt: {:?}", io.asked);
}

#[test]
fn fails_closed_when_the_account_server_is_unreachable_and_refuses_anonymous_installation() {
    let (api, _log) = mock_api(api_ok());
    let f = fx("closed", &api);
    login(&f);
    // The stored token is pinned to the API that issued it: point it at a dead address.
    let mut c = f
        .env
        .home
        .load_credentials()
        .expect("store")
        .expect("credentials");
    c.api = "http://127.0.0.1:9".to_owned();
    f.env.home.save_credentials(&c).expect("save");
    let err = run(&f, &mut Script::default(), &["install", ID]).expect_err("closed");
    assert!(
        err.contains("impossible de vérifier ton compte") && err.contains("kortex login"),
        "{err}"
    );
    assert!(
        f.env
            .home
            .load_registry()
            .expect("registry")
            .tools
            .is_empty(),
        "nothing installed while the account cannot be verified"
    );
    // Offline logout is allowed; it cannot bypass the installation account requirement.
    assert!(
        run(&f, &mut Script::default(), &["logout"])
            .expect("logout")
            .contains("Déconnecté localement")
    );
    assert!(
        run(&f, &mut Script::default(), &["install", ID])
            .expect_err("anonymous install refused")
            .contains("compte requis")
    );
    assert!(
        f.env
            .home
            .load_registry()
            .expect("registry")
            .tools
            .is_empty()
    );
}

#[test]
fn an_unreachable_account_does_not_fail_an_update_or_removal() {
    let (api, _log) = mock_api(api_ok());
    let f = fx("best-effort", &api);
    login(&f);
    run(
        &f,
        &mut Script {
            passwords: vec!["correct horse battery".into()],
            ..Script::default()
        },
        &["install", ID],
    )
    .expect("install");
    let mut offline = f.env.clone();
    offline.api = "http://127.0.0.1:9".to_owned();
    let args = vec!["remove".to_owned(), ID.to_owned()];
    let out = {
        let mut o = Vec::new();
        kortex_cli::execute_io(&args, &mut o, Some(&offline), &mut Script::default())
            .expect("remove works offline");
        String::from_utf8(o).expect("utf8")
    };
    assert!(out.contains("supprimé"));
    let _ = &f.dir;
}

#[derive(Debug)]
struct RefuseProtectedWrite;
impl kortex_platform::secrets::SecretStore for RefuseProtectedWrite {
    fn read(
        &self,
        _: &str,
        _: &str,
    ) -> Result<Option<Vec<u8>>, kortex_platform::secrets::SecretError> {
        Ok(None)
    }
    fn write(
        &self,
        _: &str,
        _: &str,
        _: &[u8],
    ) -> Result<(), kortex_platform::secrets::SecretError> {
        Err(kortex_platform::secrets::SecretError::LockedOrDenied)
    }
    fn delete(&self, _: &str, _: &str) -> Result<(), kortex_platform::secrets::SecretError> {
        Ok(())
    }
}
#[test]
fn failed_protected_persistence_revokes_the_new_device_and_never_claims_login() {
    let (api, log) = mock_api(api_ok());
    let mut f = fx("store-failed", &api);
    f.env.home = f
        .env
        .home
        .clone()
        .with_secret_store(std::sync::Arc::new(RefuseProtectedWrite));
    let mut io = Script {
        emails: vec!["a@example.com".into()],
        passwords: vec!["correct horse battery".into()],
        ..Script::default()
    };
    let error =
        run(&f, &mut io, &["login", "--name", "Test Mac"]).expect_err("protected write failed");
    assert!(error.contains("trousseau"));
    assert!(!error.contains("kdt_secret_token"));
    assert!(!f.env.home.credentials_path().exists());
    assert!(log.lock().expect("log").iter().any(|r| r.method == "DELETE"
        && r.path == "/v1/devices/self"
        && r.auth.as_deref() == Some("Bearer kdt_secret_token")));
}
