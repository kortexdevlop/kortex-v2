//! Native host backend. Package policy stays in kortex-pkg; OS calls live here.
//! This module is unavailable to browser builds and never executes commands through a shell.
use std::io::{self, Read, Write};
use std::path::{Path, PathBuf};

pub mod fs {
    use super::*;
    pub fn canonicalize(path: impl AsRef<Path>) -> io::Result<PathBuf> {
        std::fs::canonicalize(path)
    }
    pub fn read(path: impl AsRef<Path>) -> io::Result<Vec<u8>> {
        std::fs::read(path)
    }
    pub fn read_to_string(path: impl AsRef<Path>) -> io::Result<String> {
        std::fs::read_to_string(path)
    }
    pub fn write(path: impl AsRef<Path>, data: impl AsRef<[u8]>) -> io::Result<()> {
        std::fs::write(path, data)
    }
    pub fn create_dir_all(path: impl AsRef<Path>) -> io::Result<()> {
        std::fs::create_dir_all(path)
    }
    pub fn remove_dir_all(path: impl AsRef<Path>) -> io::Result<()> {
        std::fs::remove_dir_all(path)
    }
    pub fn remove_file(path: impl AsRef<Path>) -> io::Result<()> {
        std::fs::remove_file(path)
    }
    pub fn rename(from: impl AsRef<Path>, to: impl AsRef<Path>) -> io::Result<()> {
        std::fs::rename(from, to)
    }
    pub fn read_dir(path: impl AsRef<Path>) -> io::Result<std::fs::ReadDir> {
        std::fs::read_dir(path)
    }
    pub fn metadata(path: impl AsRef<Path>) -> io::Result<std::fs::Metadata> {
        std::fs::metadata(path)
    }
    pub fn exists(path: impl AsRef<Path>) -> bool {
        path.as_ref().exists()
    }
    pub fn is_dir(path: impl AsRef<Path>) -> bool {
        path.as_ref().is_dir()
    }
    /// Stream with a hard cap, including when a file changes after its metadata was read.
    pub fn read_limited(path: impl AsRef<Path>, limit: u64) -> io::Result<Vec<u8>> {
        let file = std::fs::File::open(path)?;
        let mut bytes = Vec::new();
        file.take(limit.saturating_add(1)).read_to_end(&mut bytes)?;
        if bytes.len() as u64 > limit {
            return Err(io::Error::new(
                io::ErrorKind::FileTooLarge,
                "file exceeds size limit",
            ));
        }
        Ok(bytes)
    }
    /// Create a file exclusively; never overwrite an existing path or follow its symlink.
    pub fn write_new(path: impl AsRef<Path>, bytes: &[u8]) -> io::Result<()> {
        let path = path.as_ref();
        let mut file = std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(path)?;
        if let Err(error) = file.write_all(bytes) {
            drop(file);
            let _ = std::fs::remove_file(path);
            return Err(error);
        }
        Ok(())
    }
    pub fn set_executable(path: impl AsRef<Path>) -> io::Result<()> {
        set_mode(path, 0o755)
    }
    pub fn set_private(path: impl AsRef<Path>) -> io::Result<()> {
        set_mode(path, 0o600)
    }
    fn set_mode(path: impl AsRef<Path>, mode: u32) -> io::Result<()> {
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(path, std::fs::Permissions::from_mode(mode))
        }
        #[cfg(not(unix))]
        {
            let _ = (path, mode);
            Ok(())
        }
    }
}

pub mod env {
    use super::*;
    pub fn var(key: &str) -> Result<String, std::env::VarError> {
        std::env::var(key)
    }
    pub fn var_os(key: &str) -> Option<std::ffi::OsString> {
        std::env::var_os(key)
    }
    pub fn current_exe() -> io::Result<PathBuf> {
        std::env::current_exe()
    }
    pub fn home_dir() -> Option<PathBuf> {
        var_os("HOME")
            .or_else(|| var_os("USERPROFILE"))
            .map(PathBuf::from)
    }
}

pub fn launch_executable(path: &Path) -> io::Result<()> {
    std::process::Command::new(path).spawn().map(|_| ())
}
pub fn device_name() -> String {
    env::var("HOSTNAME")
        .or_else(|_| env::var("COMPUTERNAME"))
        .ok()
        .filter(|v| !v.is_empty())
        .or_else(|| {
            std::process::Command::new("hostname")
                .output()
                .ok()
                .and_then(|v| String::from_utf8(v.stdout).ok())
                .map(|v| v.trim().to_owned())
                .filter(|v| !v.is_empty())
        })
        .unwrap_or_else(|| "kortex-cli".to_owned())
}
pub fn terminal_line(label: Option<&str>, trim: bool) -> Option<String> {
    if let Some(label) = label {
        eprint!("{label}");
        let _ = io::stderr().flush();
    }
    let mut line = String::new();
    if io::stdin().read_line(&mut line).ok()? == 0 {
        return None;
    }
    Some(if trim {
        line.trim().to_owned()
    } else {
        line.trim_end_matches(['\r', '\n']).to_owned()
    })
}
pub fn terminal_password(label: &str) -> Option<String> {
    rpassword::prompt_password(label).ok()
}

/// Save a user-requested output without races that could overwrite an existing file.
pub fn save_output(directory: &Path, name: &str, bytes: &[u8]) -> io::Result<PathBuf> {
    if matches!(name, "." | "..") {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "invalid output name",
        ));
    }
    fs::create_dir_all(directory)?;
    let base = Path::new(name)
        .file_name()
        .filter(|v| !v.is_empty())
        .map_or_else(
            || "kortex-output".to_owned(),
            |v| v.to_string_lossy().into_owned(),
        );
    if base == "." || base == ".." || base.contains(['/', '\\']) {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "invalid output name",
        ));
    }
    let (stem, extension) = base
        .rsplit_once('.')
        .map_or((base.as_str(), String::new()), |(s, e)| {
            (s, format!(".{e}"))
        });
    let mut candidate = directory.join(&base);
    let mut suffix = 2;
    loop {
        match fs::write_new(&candidate, bytes) {
            Ok(()) => return Ok(candidate),
            Err(e) if e.kind() == io::ErrorKind::AlreadyExists => {
                candidate = directory.join(format!("{stem} ({suffix}){extension}"));
                suffix += 1;
            }
            Err(e) => return Err(e),
        }
    }
}
pub fn save_download(name: &str, bytes: &[u8]) -> Result<String, String> {
    let dir = env::home_dir()
        .ok_or("dossier personnel introuvable")?
        .join("Downloads");
    save_output(&dir, name, bytes)
        .map(|p| p.display().to_string())
        .map_err(|e| e.to_string())
}

/// Bounded native HTTP request. Callers receive data, never a transport-specific client.
pub struct HttpRequest<'a> {
    pub method: &'a str,
    pub url: &'a str,
    pub headers: &'a [(&'a str, &'a str)],
    pub body: Option<&'a [u8]>,
    pub timeout: std::time::Duration,
    pub max_bytes: u64,
}
pub struct HttpResponse {
    pub status: u16,
    pub body: Vec<u8>,
}
#[must_use]
pub fn is_local_http(url: &str) -> bool {
    let Some(rest) = url.strip_prefix("http://") else {
        return false;
    };
    let authority = rest.split(['/', '?', '#']).next().unwrap_or("");
    if authority.contains('@') {
        return false;
    }
    if let Some(port) = authority.strip_prefix("[::1]") {
        return port.is_empty()
            || port
                .strip_prefix(':')
                .is_some_and(|p| !p.is_empty() && p.bytes().all(|c| c.is_ascii_digit()));
    }
    let host = authority
        .split_once(':')
        .map_or(authority, |(host, _)| host);
    matches!(host, "localhost" | "127.0.0.1")
}
pub fn http(request: HttpRequest<'_>) -> Result<HttpResponse, String> {
    let secure = request.url.starts_with("https://");
    if !secure && !is_local_http(request.url) {
        return Err("unencrypted remote HTTP is forbidden".into());
    }
    let agent: ureq::Agent = ureq::Agent::config_builder()
        .timeout_global(Some(request.timeout))
        .http_status_as_error(false)
        .https_only(secure)
        .max_redirects(if secure { 10 } else { 0 })
        .build()
        .into();
    let mut response = match request.method {
        "GET" | "DELETE" => {
            let mut builder = if request.method == "GET" {
                agent.get(request.url)
            } else {
                agent.delete(request.url)
            };
            for (name, value) in request.headers {
                builder = builder.header(*name, *value);
            }
            builder.call().map_err(|e| e.to_string())?
        }
        "POST" => {
            let mut builder = agent.post(request.url);
            for (name, value) in request.headers {
                builder = builder.header(*name, *value);
            }
            builder
                .send(request.body.unwrap_or_default())
                .map_err(|e| e.to_string())?
        }
        _ => return Err("unsupported HTTP method".into()),
    };
    let status = response.status().as_u16();
    let body = response
        .body_mut()
        .with_config()
        .limit(request.max_bytes)
        .read_to_vec()
        .map_err(|e| e.to_string())?;
    Ok(HttpResponse { status, body })
}

#[must_use]
pub fn process_id() -> u32 {
    std::process::id()
}
/// Native CLI bootstrap; command logic stays injectable and free of terminal access.
pub fn run_cli(
    execute: impl FnOnce(&[String], &mut dyn Write) -> Result<(), String>,
) -> std::process::ExitCode {
    let args = std::env::args().skip(1).collect::<Vec<_>>();
    match execute(&args, &mut io::stdout()) {
        Ok(()) => std::process::ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("erreur : {error}");
            std::process::ExitCode::FAILURE
        }
    }
}
