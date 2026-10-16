use std::time::Duration;

use crate::error::{Kind, PkgError, Result};

fn is_local_host(url: &str) -> bool {
    let rest = url.strip_prefix("http://").unwrap_or("");
    let host = rest.split(['/', ':']).next().unwrap_or("");
    host == "localhost" || host == "127.0.0.1" || host == "[::1]"
}

/// Absolute `url`, or `url` resolved against the directory of `base` (the index location).
#[must_use]
pub fn resolve(base: &str, url: &str) -> String {
    if url.contains("://") || url.starts_with('/') {
        return url.to_owned();
    }
    let dir = base.rsplit_once('/').map_or("", |(d, _)| d);
    if dir.is_empty() {
        url.to_owned()
    } else {
        format!("{dir}/{url}")
    }
}

/// Reads `src`: `https://…`, `http://localhost…` (development), `file://…` or a plain path.
/// Plain `http://` to any other host is refused, and every read is size-capped.
pub fn fetch(src: &str, max_bytes: u64) -> Result<Vec<u8>> {
    if src.starts_with("https://") || (src.starts_with("http://") && is_local_host(src)) {
        let agent: ureq::Agent = ureq::Agent::config_builder()
            .timeout_global(Some(Duration::from_secs(300)))
            .build()
            .into();
        let mut res = agent.get(src).call().map_err(|e| {
            PkgError::new(
                Kind::Network,
                format!("téléchargement impossible ({src}) : {e}"),
            )
        })?;
        return res
            .body_mut()
            .with_config()
            .limit(max_bytes)
            .read_to_vec()
            .map_err(|e| {
                PkgError::new(Kind::Network, format!("lecture interrompue ({src}) : {e}"))
            });
    }
    if src.starts_with("http://") {
        return Err(PkgError::new(
            Kind::Unsafe,
            format!("connexion non chiffrée refusée : {src}"),
        ));
    }
    let path = src.strip_prefix("file://").unwrap_or(src);
    let meta = std::fs::metadata(path)
        .map_err(|e| PkgError::new(Kind::NotFound, format!("{path} : {e}")))?;
    if meta.len() > max_bytes {
        return Err(PkgError::new(
            Kind::Unsafe,
            format!("{path} dépasse la taille maximale autorisée"),
        ));
    }
    Ok(std::fs::read(path)?)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn relative_urls_resolve_against_the_index_directory() {
        assert_eq!(
            resolve("https://kortex.site/registry/index.json", "pkg/a.tar.gz"),
            "https://kortex.site/registry/pkg/a.tar.gz"
        );
        assert_eq!(
            resolve("/tmp/rel/index.json", "a.tar.gz"),
            "/tmp/rel/a.tar.gz"
        );
        assert_eq!(
            resolve("https://x/i.json", "https://cdn/a.tgz"),
            "https://cdn/a.tgz"
        );
        assert_eq!(resolve("index.json", "a.tgz"), "a.tgz");
    }

    #[test]
    fn plain_http_to_remote_hosts_is_refused() {
        let e = fetch("http://evil.example/a", 10).expect_err("refused");
        assert_eq!(e.kind(), Kind::Unsafe);
        assert!(is_local_host("http://localhost:8080/x") && is_local_host("http://127.0.0.1/x"));
        assert!(!is_local_host("http://localhost.evil.example/x"));
    }

    #[test]
    fn files_are_size_capped() {
        let p = std::env::temp_dir().join(format!("kortex-pkg-net-{}", std::process::id()));
        std::fs::write(&p, vec![0_u8; 100]).expect("write");
        assert!(fetch(p.to_str().expect("path"), 1000).is_ok());
        assert_eq!(
            fetch(p.to_str().expect("path"), 10)
                .expect_err("too big")
                .kind(),
            Kind::Unsafe
        );
        assert_eq!(
            fetch("/definitely/missing", 10)
                .expect_err("missing")
                .kind(),
            Kind::NotFound
        );
    }
}
