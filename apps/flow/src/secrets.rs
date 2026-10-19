//! Secrets for HTTP nodes (API tokens). They live on this device only, are referenced from a
//! workflow as `{secret:NAME}` and are never part of the workflow JSON or any export. Desktop keeps
//! the values in the system credential store; the browser keeps them in this origin's local
//! storage (readable by anyone using this browser profile: say so in the UI).
use kortex_shell as shell;

const NAMES: &str = "secret-names";

/// A secret name as written in `{secret:NAME}`.
#[must_use]
pub fn valid_name(name: &str) -> bool {
    (1..=40).contains(&name.len())
        && name
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-')
}

/// The names that have a value on this device (never the values).
#[must_use]
pub fn names() -> Vec<String> {
    shell::load::<Vec<String>>("flow", NAMES).0
}

#[cfg(not(target_arch = "wasm32"))]
const SERVICE: &str = "kortex-flow";

#[must_use]
pub fn get(name: &str) -> Option<String> {
    if !valid_name(name) {
        return None;
    }
    #[cfg(target_arch = "wasm32")]
    {
        shell::load::<Option<String>>("flow", &format!("secret.{name}")).0
    }
    #[cfg(not(target_arch = "wasm32"))]
    {
        use kortex_platform::secrets::{NativeSecretStore, SecretStore};
        NativeSecretStore
            .read(SERVICE, name)
            .ok()
            .flatten()
            .and_then(|bytes| String::from_utf8(bytes).ok())
    }
}

/// Stores or replaces a secret.
///
/// # Errors
/// `invalid_name`, `empty_value`, or `storage` when the store refuses.
pub fn set(name: &str, value: &str) -> Result<(), &'static str> {
    if !valid_name(name) {
        return Err("invalid_name");
    }
    if value.is_empty() || value.len() > 4_000 {
        return Err("empty_value");
    }
    #[cfg(target_arch = "wasm32")]
    let stored = shell::save("flow", &format!("secret.{name}"), &Some(value.to_owned()));
    #[cfg(not(target_arch = "wasm32"))]
    let stored = {
        use kortex_platform::secrets::{NativeSecretStore, SecretStore};
        NativeSecretStore
            .write(SERVICE, name, value.as_bytes())
            .is_ok()
    };
    if !stored {
        return Err("storage");
    }
    let mut all = names();
    if !all.iter().any(|n| n == name) {
        all.push(name.to_owned());
        all.sort();
        shell::save("flow", NAMES, &all);
    }
    Ok(())
}

pub fn remove(name: &str) {
    #[cfg(target_arch = "wasm32")]
    {
        shell::save("flow", &format!("secret.{name}"), &None::<String>);
    }
    #[cfg(not(target_arch = "wasm32"))]
    {
        use kortex_platform::secrets::{NativeSecretStore, SecretStore};
        let _ = NativeSecretStore.delete(SERVICE, name);
    }
    let mut all = names();
    all.retain(|n| n != name);
    shell::save("flow", NAMES, &all);
}
