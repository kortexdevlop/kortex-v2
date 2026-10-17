//! Kortex package manager core, shared by the `kortex` CLI and the Kortex Store.
//!
//! Trust model: a signed index (`index.json` + `index.json.sig`, Ed25519) lists, for every release
//! artifact, its URL, size and SHA-256. The signature is checked against a public key compiled into
//! the client; artifacts are only accepted if they match the signed hash. Archives are extracted
//! through an allow-list (never trusting entry names), so a malicious package cannot write outside
//! its own directory.
mod account;
mod env;
mod error;
mod home;
mod index;
mod install;
mod net;
pub mod shortcuts;
mod version;

pub use account::{
    Credentials, DEFAULT_API, KDF_ITERATIONS, LogoutOutcome, confirm_password, confirm_required,
    derive_auth_key, forget_install, login_device, logout, record_install, whoami,
};
pub use env::{DEFAULT_INDEX, PkgEnv};
pub use error::{Kind, PkgError};
pub use home::{Home, InstalledTool, Registry};
pub use index::{
    Artifact, CliRelease, Index, Manifest, ToolRelease, parse_verified, public_key_b64, sha256_hex,
    sign_bytes,
};
pub use install::{
    Check, ToolStatus, binary_name_cli, doctor, install, installed_binary, remove, self_update,
    statuses, update,
};
pub use net::{fetch, resolve};
pub use version::{Version, is_newer};

/// Release targets we ship for.
pub const TARGETS: [&str; 4] = [
    "x86_64-unknown-linux-gnu",
    "aarch64-apple-darwin",
    "x86_64-apple-darwin",
    "x86_64-pc-windows-msvc",
];

/// The release target of the running binary, if we ship for it.
#[must_use]
pub fn current_target() -> Option<&'static str> {
    match (std::env::consts::OS, std::env::consts::ARCH) {
        ("linux", "x86_64") => Some(TARGETS[0]),
        ("macos", "aarch64") => Some(TARGETS[1]),
        ("macos", "x86_64") => Some(TARGETS[2]),
        ("windows", "x86_64") => Some(TARGETS[3]),
        _ => None,
    }
}

/// File name of a tool's desktop binary inside its package.
#[must_use]
pub fn binary_name(id: &str, target: &str) -> String {
    let ext = if target.contains("windows") {
        ".exe"
    } else {
        ""
    };
    format!("kortex-{}{ext}", id.replace('.', "-"))
}

/// Public key of the release signing key (Ed25519, raw 32 bytes, base64).
/// Regenerate with `cargo xtask keygen`; the private half never enters the repository.
pub const RELEASE_PUBLIC_KEY: &str = include_str!("../release.pub");
