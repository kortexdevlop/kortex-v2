//! Shared application frame, contract catalogue and tool host. No tool engine imports.
use dioxus::prelude::*;
use kortex_ui::Button;
use serde::Deserialize;
use std::sync::OnceLock;

pub mod account;
pub mod catalog;
pub use account::AccountSync;
pub use catalog::{Catalogue, Source};

pub const CSS: &str = include_str!("shell.css");
#[derive(Clone, PartialEq, Deserialize)]
pub struct Entry {
    pub tool: kortex_contract::ToolMeta,
    pub search: kortex_contract::Search,
    #[serde(default)]
    pub variants: Vec<kortex_contract::Variant>,
    #[serde(default)]
    pub inputs: Vec<kortex_contract::Input>,
    #[serde(default)]
    pub params: Vec<kortex_contract::Param>,
    #[serde(default)]
    pub outputs: Vec<kortex_contract::Output>,
    #[serde(default)]
    pub permissions: Option<kortex_contract::Permissions>,
}
/// The catalogue in use: verified published catalogue, else its verified offline copy, else the
/// contracts compiled into this build (ADR 0034). Components should call [`use_catalogue`] to
/// re-render when a newer verified catalogue arrives.
#[must_use]
pub fn catalogue() -> Catalogue {
    catalog::current()
}
pub fn name(entry: &Entry, lang: &str) -> String {
    entry
        .tool
        .name
        .get(lang)
        .cloned()
        .unwrap_or_else(|| entry.tool.id.clone())
}
/// Full contracts of the catalogue in use, by id; recomputed only when the catalogue changes.
#[must_use]
pub fn contracts()
-> std::sync::Arc<std::collections::BTreeMap<String, kortex_contract::ToolContract>> {
    type Map = std::collections::BTreeMap<String, kortex_contract::ToolContract>;
    static MEMO: std::sync::Mutex<Option<(Catalogue, std::sync::Arc<Map>)>> =
        std::sync::Mutex::new(None);
    let current = catalogue();
    let mut memo = MEMO
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    if let Some((seen, map)) = memo.as_ref()
        && seen.same(&current)
    {
        return std::sync::Arc::clone(map);
    }
    let map: std::sync::Arc<Map> = std::sync::Arc::new(
        current
            .contracts()
            .into_iter()
            .map(|c| (c.tool.id.clone(), c))
            .collect(),
    );
    *memo = Some((current, std::sync::Arc::clone(&map)));
    map
}
#[must_use]
pub fn find(id: &str) -> Option<Entry> {
    catalogue().find(id).cloned()
}
/// Bumped when a newer verified catalogue replaces the one in use.
static REVISION: GlobalSignal<u64> = Signal::global(|| 0);
/// The catalogue in use, re-rendering the component when a newer verified one arrives. The first
/// call fetches the published catalogue once, in the background; failures keep the current one.
pub fn use_catalogue() -> Catalogue {
    use std::sync::atomic::{AtomicBool, Ordering};
    static STARTED: AtomicBool = AtomicBool::new(false);
    use_hook(|| {
        if !STARTED.swap(true, Ordering::SeqCst) {
            dioxus::core::spawn_forever(async {
                if let Some(next) = catalog::fetch().await
                    && catalog::replace(next)
                {
                    *REVISION.write() += 1;
                }
            });
        }
    });
    let _ = REVISION();
    catalogue()
}
/// Number of catalogue replacements so far; reading it inside a memo or effect re-runs it when a
/// newer verified catalogue arrives.
#[must_use]
pub fn catalogue_revision() -> u64 {
    REVISION()
}
/// Tools an app may offer: the whole catalogue on the web; on desktop only the tools really
/// installed (RULES §4.7), including ones this build does not know, described by the contract
/// saved with their package.
pub fn available() -> Vec<Entry> {
    #[cfg(target_arch = "wasm32")]
    {
        catalogue().to_vec()
    }
    #[cfg(not(target_arch = "wasm32"))]
    {
        let catalogue = catalogue();
        let Some(registry) = kortex_pkg::Home::from_env()
            .ok()
            .and_then(|h| h.load_registry().ok().map(|r| (h, r)))
        else {
            return Vec::new();
        };
        let (home, registry) = registry;
        registry
            .tools
            .iter()
            .filter(|(id, _)| installed_path(id).is_some())
            .filter_map(|(id, item)| {
                catalogue
                    .find(id)
                    .cloned()
                    .or_else(|| installed_contract(&home, id, &item.version))
            })
            .collect()
    }
}
/// Contract saved next to an installed package (`contract.json`, written at installation).
#[cfg(not(target_arch = "wasm32"))]
fn installed_contract(home: &kortex_pkg::Home, id: &str, version: &str) -> Option<Entry> {
    let path = home
        .tools_dir()
        .join(id)
        .join(version)
        .join("contract.json");
    let bytes = kortex_platform::native::fs::read(path).ok()?;
    let entry: Entry = serde_json::from_slice(&bytes).ok()?;
    (entry.tool.id == id).then_some(entry)
}
#[cfg(not(target_arch = "wasm32"))]
fn installed_path(id: &str) -> Option<std::path::PathBuf> {
    let home = kortex_pkg::Home::from_env().ok()?;
    let registry = home.load_registry().ok()?;
    let item = registry.tools.get(id)?;
    let target = kortex_pkg::current_target()?;
    let expected = std::path::PathBuf::from("tools")
        .join(id)
        .join(&item.version)
        .join(kortex_pkg::binary_name(id, target));
    if std::path::Path::new(&item.binary) != expected {
        return None;
    }
    let path = home.root().join(expected);
    let canonical = kortex_platform::native::fs::canonicalize(&path).ok()?;
    let tools = kortex_platform::native::fs::canonicalize(home.tools_dir()).ok()?;
    (canonical.starts_with(tools)
        && kortex_platform::native::fs::metadata(&canonical)
            .ok()?
            .is_file())
    .then_some(canonical)
}
/// Verified path of an installed tool binary (desktop only; `None` on the web or when absent).
#[cfg(not(target_arch = "wasm32"))]
#[must_use]
pub fn installed_binary(id: &str) -> Option<std::path::PathBuf> {
    installed_path(id)
}
pub fn launch_installed(id: &str) -> bool {
    #[cfg(target_arch = "wasm32")]
    {
        let _ = id;
        false
    }
    #[cfg(not(target_arch = "wasm32"))]
    {
        installed_path(id).is_some_and(|p| kortex_platform::native::launch_executable(&p).is_ok())
    }
}
pub fn matches(entry: &Entry, query: &str, lang: &str) -> bool {
    let query = fold(query);
    if query.is_empty() {
        return true;
    }
    let text = fold(&format!(
        "{} {} {}",
        name(entry, lang),
        entry.tool.universe,
        entry
            .search
            .keywords
            .get(lang)
            .cloned()
            .unwrap_or_default()
            .join(" ")
    ));
    query.split_whitespace().all(|word| text.contains(word))
}
fn fold(text: &str) -> String {
    text.to_lowercase()
        .chars()
        .map(|c| match c {
            'é' | 'è' | 'ê' | 'ë' => 'e',
            'à' | 'â' | 'ä' => 'a',
            'î' | 'ï' => 'i',
            'ô' | 'ö' => 'o',
            'ù' | 'û' | 'ü' => 'u',
            'ç' => 'c',
            _ => c,
        })
        .collect()
}
pub fn domain(key: &str) -> String {
    static CONFIG: OnceLock<toml::Table> = OnceLock::new();
    CONFIG
        .get_or_init(|| {
            toml::from_str(include_str!("../../../brand/domains.toml")).unwrap_or_default()
        })
        .get("domains")
        .and_then(|v| v.get(key))
        .and_then(toml::Value::as_str)
        .unwrap_or_default()
        .to_owned()
}
pub fn url(key: &str, lang: &str) -> String {
    format!(
        "https://{}/{}",
        domain(key),
        if lang == "en" { "en/" } else { "" }
    )
}
pub fn load<T: serde::de::DeserializeOwned + Default>(app: &str, key: &str) -> (T, bool) {
    match kortex_platform::document::preference_get(app, key) {
        Ok(Some(value)) => serde_json::from_str(&value)
            .map(|v| (v, false))
            .unwrap_or_else(|_| (T::default(), true)),
        Ok(None) => (T::default(), false),
        Err(_) => (T::default(), true),
    }
}
pub fn save<T: serde::Serialize>(app: &str, key: &str, value: &T) -> bool {
    serde_json::to_string(value)
        .ok()
        .is_some_and(|value| kortex_platform::document::preference_set(app, key, &value).is_ok())
}
#[derive(Clone, Copy)]
pub struct Appearance {
    pub lang: Signal<String>,
    pub theme: Signal<String>,
    pub mode: Signal<String>,
    pub size: Signal<String>,
}
/// Remembers a shared preference (cookie on the web) only once it differs from the default,
/// so merely opening a page never sets a cookie.
fn remember(key: &str, value: &str) {
    let default = match key {
        "mode" => "medium",
        "size" => "normal",
        _ => "",
    };
    if value != default || kortex_platform::document::shared_get(key).is_some() {
        kortex_platform::document::shared_set(&domain("root"), key, value);
    }
}
pub fn use_appearance() -> Appearance {
    // Installable PWA: the offline shell of this app (generated at build time, same-origin only).
    #[cfg(target_arch = "wasm32")]
    use_hook(|| {
        kortex_platform::offline::register("/sw.js");
        kortex_platform::offline::report_errors(&format!("https://{}", domain("api")));
    });
    let lang = use_signal(|| {
        let initial = kortex_platform::document::initial_language();
        let saved = load::<String>("shell", "lang").0;
        if initial == "en" {
            initial
        } else if ["fr", "en"].contains(&saved.as_str()) {
            saved
        } else {
            initial
        }
    });
    // The shared cookie (set on any Kortex host) wins over this host's own saved copy.
    let theme = use_signal(|| {
        kortex_platform::document::shared_get("theme")
            .unwrap_or_else(|| load::<String>("shell", "theme").0)
    });
    let mode = use_signal(|| {
        let x = kortex_platform::document::shared_get("mode")
            .unwrap_or_else(|| load::<String>("shell", "mode").0);
        if ["simple", "medium", "pro"].contains(&x.as_str()) {
            x
        } else {
            "medium".into()
        }
    });
    let size = use_signal(|| {
        kortex_platform::document::shared_get("size").unwrap_or_else(|| "normal".into())
    });
    use_context_provider(|| Appearance {
        lang,
        theme,
        mode,
        size,
    });
    use_effect(move || {
        kortex_platform::document::apply_theme(&theme());
        kortex_platform::document::apply_size(&size());
        kortex_platform::document::apply_language(&lang());
        save("shell", "lang", &lang());
        save("shell", "theme", &theme());
        save("shell", "mode", &mode());
        remember("theme", &theme());
        remember("mode", &mode());
        remember("size", &size());
    });
    Appearance {
        lang,
        theme,
        mode,
        size,
    }
}
#[component]
pub fn Frame(title: String, subtitle: String, children: Element) -> Element {
    let mut appearance = use_context::<Appearance>();
    let lang = (appearance.lang)();
    let fr = lang == "fr";
    let mode = (appearance.mode)();
    let dark = (appearance.theme)() == "dark";
    let large = (appearance.size)() == "large";
    let links = [
        (url("hub", &lang), "Kortex Hub", "hub"),
        (url("desk", &lang), "Kortex Desk", "desk"),
        (
            format!(
                "{}{}",
                url("root", &lang),
                if fr { "outils" } else { "tools" }
            ),
            if fr { "Tous les outils" } else { "All tools" },
            "tools",
        ),
        (
            format!(
                "{}{}",
                url("root", &lang),
                if fr { "compte" } else { "account" }
            ),
            if fr { "Compte" } else { "Account" },
            "account",
        ),
    ];
    rsx! {
     style { {include_str!("../../../design/dist/kortex.css")} }
        style { "{CSS}" }
     div {class:"k-ambient","aria-hidden":"true"}
     div {class:"app-shell",
      aside {class:"app-rail",
       a {class:"app-brand",href:url("root",&lang),
        span {class:"app-brand-mark","aria-hidden":"true",
         svg {view_box:"0 0 64 64",fill:"none",stroke:"currentColor",stroke_width:"5",stroke_linecap:"round",stroke_linejoin:"round",
          path {d:"M22 13V51M22 33C30 33 37 28 43 15C46 25 40 32 33 36C41 38 46 44 47 51"}
         }
        }
        "Kortex"
       }
       nav { "aria-label":if fr{"Applications"}else{"Apps"},
        for (href , label , icon) in links {
         a { key:"{icon}", href:"{href}",
          svg {class:"app-nav-icon",view_box:"0 0 24 24","aria-hidden":"true",
           match icon {
            "hub" => rsx!{ rect{x:"4",y:"4",width:"7",height:"9",rx:"2"} rect{x:"13",y:"4",width:"7",height:"5",rx:"2"} rect{x:"4",y:"15",width:"7",height:"5",rx:"2"} rect{x:"13",y:"11",width:"7",height:"9",rx:"2"} },
            "desk" => rsx!{ rect{x:"3.5",y:"5",width:"17",height:"12",rx:"2.5"} path{d:"M8 20h8M12 17v3"} },
            "tools" => rsx!{ rect{x:"4",y:"4",width:"6.5",height:"6.5",rx:"2"} rect{x:"13.5",y:"4",width:"6.5",height:"6.5",rx:"2"} rect{x:"4",y:"13.5",width:"6.5",height:"6.5",rx:"2"} rect{x:"13.5",y:"13.5",width:"6.5",height:"6.5",rx:"3.25"} },
            _ => rsx!{ circle{cx:"12",cy:"9",r:"3.5"} path{d:"M5.5 19.5c1.3-3 3.6-4.5 6.5-4.5s5.2 1.5 6.5 4.5"} },
           }
          }
          span {"{label}"}
         }
        }
       }
       p {class:"app-rail-note k-muted",if fr{"Vos fichiers restent sur cet appareil."}else{"Your files stay on this device."}}
      }
      main {class:"app-main",
       header {class:"app-header",
        div {class:"app-heading", h1{"{title}"} p{class:"k-muted","{subtitle}"}}
        div {class:"app-settings",
         div {class:"k-segmented",role:"radiogroup","aria-label":"Mode",
          for (id , label) in [("simple","Simple"),("medium","Medium"),("pro","Pro")] {
           button {key:"{id}",class:"k-segment",r#type:"button",role:"radio","aria-checked":"{mode == id}",
            onclick:move |_| { let mut m = appearance.mode; m.set(id.into()); },
            "{label}"
           }
          }
         }
         button {class:"k-icon-btn app-icon",r#type:"button","aria-label":if fr{"Thème"}else{"Theme"},title:if fr{"Thème"}else{"Theme"},
          onclick:move |_| {let next=if (appearance.theme)()=="dark"{"light"}else{"dark"};appearance.theme.set(next.into());},
          svg {view_box:"0 0 24 24","aria-hidden":"true",
           if dark {
            circle{cx:"12",cy:"12",r:"4"} path{d:"M12 2.5v2M12 19.5v2M4.6 4.6l1.4 1.4M18 18l1.4 1.4M2.5 12h2M19.5 12h2M4.6 19.4 6 18M18 6l1.4-1.4"}
           } else {
            path{d:"M20 14.5A8 8 0 1 1 9.5 4a6.5 6.5 0 0 0 10.5 10.5Z"}
           }
          }
         }
         button {class:"k-icon-btn app-icon app-size",r#type:"button","aria-pressed":"{large}","aria-label":if fr{"Texte plus grand"}else{"Larger text"},title:if fr{"Texte plus grand"}else{"Larger text"},
          onclick:move |_| {let next=if (appearance.size)()=="large"{"normal"}else{"large"};appearance.size.set(next.into());},
          "Aa"
         }
         button {class:"k-icon-btn app-icon app-lang",r#type:"button",lang:if fr{"en"}else{"fr"},
          onclick:move |_|{let next=if (appearance.lang)()=="fr"{"en"}else{"fr"};appearance.lang.set(next.into());},
          if fr{"EN"}else{"FR"}
         }
        }
       }
       {children}
      }
     }
    }
}
#[component]
pub fn ToolHost(id: String, frame: u64) -> Element {
    let appearance = use_context::<Appearance>();
    let lang = (appearance.lang)();
    let entry = find(&id);
    let title = entry.map(|e| name(&e, &lang)).unwrap_or_else(|| id.clone());
    #[cfg(target_arch = "wasm32")]
    {
        let origin = format!("https://{}", domain("run"));
        let source =
            use_hook(|| format!("{origin}/{id}/?lang={lang}&mode={}", (appearance.mode)()));
        let mut ready = use_signal(|| false);
        let mut channel = use_hook(|| kortex_platform::document::tool_host_events(frame, &origin));
        use_future(move || async move {
            while let Ok(event) = channel.recv::<kortex_bus::Ready>().await {
                if event.accepts(frame) {
                    ready.set(true);
                }
            }
        });
        use_drop(move || {
            let _ = channel.send("stop");
        });
        use_effect(move || {
            let _ = ready();
            let init = kortex_bus::Init {
                lang: (appearance.lang)(),
                theme: (appearance.theme)(),
                mode: (appearance.mode)(),
            };
            kortex_platform::document::tool_host_init(frame, &origin, &init);
        });
        rsx! {div {class:"tool-surface","data-ready":ready(),
         if !ready(){p {class:"tool-loading k-muted",role:"status",if lang=="fr"{"Chargement de l’outil…"}else{"Loading tool…"}}}
         iframe {title,src:source,allow:"clipboard-write","data-kortex-window":"{frame}","data-kortex-mode":(appearance.mode)(),"data-kortex-lang":(appearance.lang)(),referrerpolicy:"origin"}
        }}
    }
    #[cfg(not(target_arch = "wasm32"))]
    {
        let _ = frame;
        let mut failed = use_signal(|| false);
        rsx! {div {class:"native-tool k-stack", h3{"{title}"}p{class:"k-muted",if lang=="fr"{"Cet outil s’ouvre dans une fenêtre séparée. Fermer cette carte ne ferme pas l’outil."}else{"This tool opens in a separate native window. Closing this card does not close the tool."}}
         Button{onclick:move |_|failed.set(!launch_installed(&id)),if lang=="fr"{"Ouvrir l’outil installé"}else{"Open installed tool"}}
         if failed(){p{class:"k-error",role:"alert",if lang=="fr"{"Ouverture impossible. Vérifiez l’installation."}else{"Unable to open. Check the installation."}}}
        }}
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn catalogue_and_search_are_real_and_local() {
        let registry: serde_json::Value =
            serde_json::from_str(include_str!("../../../registry/contracts.json"))
                .unwrap_or_default();
        assert_eq!(
            catalogue().len(),
            registry["tools"].as_array().map_or(0, Vec::len)
        );
        let e = find("secure.hash-calculator").expect("tool");
        assert!(matches(&e, "hash", "en"));
        assert!(!matches(&e, "nonexistentneedle", "fr"));
        assert!(!domain("desk").is_empty());
    }
}
