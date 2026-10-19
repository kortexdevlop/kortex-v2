//! Shared application frame, contract catalogue and tool host. No tool engine imports.
use dioxus::prelude::*;
use kortex_ui::Button;
use serde::Deserialize;
use std::sync::OnceLock;

pub mod account;
pub use account::AccountSync;

pub const CSS: &str = include_str!("shell.css");
#[derive(Clone, Deserialize)]
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
}
#[derive(Deserialize)]
struct Registry {
    tools: Vec<Entry>,
}
pub fn catalogue() -> &'static [Entry] {
    static ITEMS: OnceLock<Vec<Entry>> = OnceLock::new();
    ITEMS.get_or_init(|| {
        serde_json::from_str::<Registry>(include_str!("../../../registry/index.json"))
            .map(|r| r.tools)
            .unwrap_or_default()
    })
}
pub fn name(entry: &Entry, lang: &str) -> String {
    entry
        .tool
        .name
        .get(lang)
        .cloned()
        .unwrap_or_else(|| entry.tool.id.clone())
}
pub fn find(id: &str) -> Option<&'static Entry> {
    catalogue().iter().find(|e| e.tool.id == id)
}
pub fn available() -> Vec<Entry> {
    #[cfg(target_arch = "wasm32")]
    {
        catalogue().to_vec()
    }
    #[cfg(not(target_arch = "wasm32"))]
    {
        catalogue()
            .iter()
            .filter(|e| installed_path(&e.tool.id).is_some())
            .cloned()
            .collect()
    }
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
    let theme = use_signal(|| load::<String>("shell", "theme").0);
    let mode = use_signal(|| {
        let x = load::<String>("shell", "mode").0;
        if ["simple", "medium", "pro"].contains(&x.as_str()) {
            x
        } else {
            "medium".into()
        }
    });
    use_context_provider(|| Appearance { lang, theme, mode });
    use_effect(move || {
        kortex_platform::document::apply_theme(&theme());
        kortex_platform::document::apply_language(&lang());
        save("shell", "lang", &lang());
        save("shell", "theme", &theme());
        save("shell", "mode", &mode());
    });
    Appearance { lang, theme, mode }
}
#[component]
pub fn Frame(title: String, subtitle: String, children: Element) -> Element {
    let mut appearance = use_context::<Appearance>();
    let lang = (appearance.lang)();
    let fr = lang == "fr";
    rsx! {
     style { {include_str!("../../../design/dist/kortex.css")} }
        style { "{CSS}" }
     div {class:"app-shell",
      aside {class:"app-rail",
       a {class:"app-brand",href:url("root",&lang), "Kortex"}
       nav { "aria-label":if fr{"Applications"}else{"Apps"},
        a { href:url("hub",&lang), "Kortex Hub" }
        a { href:url("desk",&lang), "Kortex Desk" }
        a { href:format!("{}{}",url("root",&lang),if fr{"outils"}else{"tools"}), if fr{"Tous les outils"}else{"All tools"} }
        a { href:format!("{}{}",url("root",&lang),if fr{"compte"}else{"account"}), if fr{"Compte"}else{"Account"} }
       }
       p {class:"app-rail-note k-muted",if fr{"Vos fichiers restent sur cet appareil."}else{"Your files stay on this device."}}
      }
      main {class:"app-main",
       header {class:"app-header",div {p{class:"app-eyebrow",if fr{"KORTEX · FICHIERS LOCAUX"}else{"KORTEX · LOCAL FIRST"}} h1{"{title}"} p{class:"k-muted","{subtitle}"}}
        div {class:"app-settings",
         select {class:"k-input","aria-label":"Mode",value:(appearance.mode)(),onchange:move|e|appearance.mode.set(e.value()),option{value:"simple","Simple"}option{value:"medium","Medium"}option{value:"pro","Pro"}}
         Button {ghost:true,onclick:move |_| {let next=if (appearance.theme)()=="dark"{"light"}else{"dark"};appearance.theme.set(next.into());},if fr{"Thème"}else{"Theme"}}
         Button {ghost:true,onclick:move |_|{let next=if (appearance.lang)()=="fr"{"en"}else{"fr"};appearance.lang.set(next.into());},if fr{"EN"}else{"FR"}}
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
    let title = entry.map(|e| name(e, &lang)).unwrap_or_else(|| id.clone());
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
        assert_eq!(catalogue().len(), 38);
        let e = find("secure.hash-calculator").expect("tool");
        assert!(matches(e, "hash", "en"));
        assert!(!matches(e, "nonexistentneedle", "fr"));
        assert!(!domain("desk").is_empty());
    }
}
