//! Native installation for the desktop Store: the same signed-index, account and confirmation
//! rules as `kortex install`, run off the UI thread. Not compiled for the web.
use crate::engine::{Action, Outcome, Snapshot, ToolStatus};

fn convert(s: kortex_pkg::ToolStatus) -> ToolStatus {
    ToolStatus {
        id: s.id,
        name: s.name,
        universe: s.universe,
        available: s.available,
        installed: s.installed,
        update_available: s.update_available,
        supported: s.supported,
    }
}

/// Loads the signed index, the local registry and the linked account (blocking).
#[must_use]
pub fn snapshot() -> Snapshot {
    let env = match kortex_pkg::PkgEnv::from_process() {
        Ok(env) => env,
        Err(e) => {
            return Snapshot {
                error: Some(e.to_string()),
                ..Snapshot::default()
            };
        }
    };
    let account = env.home.load_credentials().ok().flatten().map(|c| c.email);
    let registry = env.home.load_registry().unwrap_or_default();
    match env.load_index() {
        Ok(index) => Snapshot {
            items: kortex_pkg::statuses(&index, &registry, env.target.as_deref())
                .into_iter()
                .map(convert)
                .collect(),
            account,
            error: None,
        },
        // Offline: installed items can still be opened or removed.
        Err(e) => Snapshot {
            items: registry
                .tools
                .into_iter()
                .map(|(id, t)| ToolStatus {
                    name: std::collections::BTreeMap::new(),
                    universe: id
                        .split_once('.')
                        .map_or_else(String::new, |(u, _)| u.to_owned()),
                    available: t.version.clone(),
                    installed: Some(t.version),
                    update_available: false,
                    supported: true,
                    id,
                })
                .collect(),
            account,
            error: Some(e.to_string()),
        },
    }
}

fn display(index: &kortex_pkg::Index, id: &str) -> String {
    index
        .tools
        .get(id)
        .and_then(|t| t.name.get("en").cloned())
        .unwrap_or_else(|| id.to_owned())
}

/// Installs, updates or removes `id` (blocking). Installing needs the linked account and, when the
/// account asks for it, its password; updates and removals follow the CLI (no confirmation).
#[must_use]
pub fn perform(id: &str, action: Action, password: Option<&str>) -> Outcome {
    match run(id, action, password) {
        Ok(outcome) => outcome,
        Err(e) => Outcome::Failed(e.to_string()),
    }
}

fn run(id: &str, action: Action, password: Option<&str>) -> Result<Outcome, kortex_pkg::PkgError> {
    let env = kortex_pkg::PkgEnv::from_process()?;
    let target = env.target()?.to_owned();
    let creds = env.home.load_credentials()?;
    let mut quiet = |_: &str| {};
    match action {
        Action::Install => {
            let Some(c) = creds else {
                return Ok(Outcome::SignedOut);
            };
            if kortex_pkg::confirm_required(&c)? {
                let Some(pw) = password.filter(|p| !p.is_empty()) else {
                    return Ok(Outcome::NeedsPassword);
                };
                if !kortex_pkg::confirm_password(&c, pw, kortex_pkg::KDF_ITERATIONS)? {
                    return Ok(Outcome::WrongPassword);
                }
            }
            let index = env.load_index()?;
            let t =
                kortex_pkg::install(&env.home, &index, &env.index_src, id, &target, &mut quiet)?;
            env.save_contract(id, &t.version);
            // The installation itself succeeded; the account record is informative only.
            let _ = kortex_pkg::record_install(&c, id, &t.version, &target);
            Ok(Outcome::Done)
        }
        Action::Update => {
            let index = env.load_index()?;
            if let Some(t) =
                kortex_pkg::update(&env.home, &index, &env.index_src, id, &target, &mut quiet)?
            {
                env.save_contract(id, &t.version);
                if let Some(c) = &creds {
                    let _ = kortex_pkg::record_install(c, id, &t.version, &target);
                }
            }
            Ok(Outcome::Done)
        }
        Action::Remove => {
            let name = env
                .load_index()
                .map_or_else(|_| id.to_owned(), |index| display(&index, id));
            kortex_pkg::remove(&env.home, id, &name)?;
            if let Some(c) = &creds {
                let _ = kortex_pkg::forget_install(c, id);
            }
            Ok(Outcome::Done)
        }
        Action::Open => Ok(if kortex_shell::launch_installed(id) {
            Outcome::Done
        } else {
            Outcome::Failed(String::new())
        }),
    }
}

/// Runs a blocking job on a background thread and awaits its result from the UI.
pub async fn background<T: Send + 'static>(job: impl FnOnce() -> T + Send + 'static) -> Option<T> {
    let slot = kortex_platform::native::task::Slot::new();
    let done = slot.clone();
    kortex_platform::native::task::spawn("kortex-store", move || {
        done.complete(job());
    })
    .ok()?;
    slot.wait().await
}
