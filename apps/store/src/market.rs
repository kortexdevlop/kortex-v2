//! Shared Store application. Release installation stays closed until secure native delivery passes.
use dioxus::prelude::*;
use kortex_shell::{Frame, catalogue, matches, name, url, use_appearance};
use kortex_ui::Button;

#[derive(Clone, serde::Deserialize)]
struct Permissions {
    network: bool,
    filesystem: String,
    clipboard: String,
    sensitive: bool,
}
fn permissions(id: &str) -> Option<Permissions> {
    #[derive(serde::Deserialize)]
    struct Tool {
        id: String,
    }
    #[derive(serde::Deserialize)]
    struct Entry {
        tool: Tool,
        permissions: Permissions,
    }
    #[derive(serde::Deserialize)]
    struct Registry {
        tools: Vec<Entry>,
    }
    static REGISTRY: std::sync::OnceLock<Vec<Entry>> = std::sync::OnceLock::new();
    REGISTRY
        .get_or_init(|| {
            serde_json::from_str::<Registry>(include_str!("../../../registry/index.json"))
                .map(|r| r.tools)
                .unwrap_or_default()
        })
        .iter()
        .find(|e| e.tool.id == id)
        .map(|e| e.permissions.clone())
}

fn installed() -> Vec<(String, String)> {
    #[cfg(target_arch = "wasm32")]
    {
        Vec::new()
    }
    #[cfg(not(target_arch = "wasm32"))]
    {
        kortex_pkg::Home::from_env()
            .ok()
            .and_then(|home| home.load_registry().ok())
            .map(|registry| {
                registry
                    .tools
                    .into_iter()
                    .filter_map(|(id, item)| {
                        kortex_pkg::Home::from_env()
                            .ok()
                            .and_then(|home| kortex_pkg::installed_binary(&home, &id))
                            .map(|_| (id, item.version))
                    })
                    .collect()
            })
            .unwrap_or_default()
    }
}

#[component]
pub fn App() -> Element {
    let appearance = use_appearance();
    let lang = (appearance.lang)();
    let fr = lang == "fr";
    let mut query = use_signal(String::new);
    let mut universe = use_signal(String::new);
    let mut selected = use_signal(|| None::<String>);
    let mut only_installed = use_signal(|| false);
    let mut installed_items = use_signal(installed);
    let mut launch_failed = use_signal(|| false);
    let is_web = cfg!(target_arch = "wasm32");
    let mut universes: Vec<_> = catalogue()
        .iter()
        .map(|e| e.tool.universe.clone())
        .collect();
    universes.sort();
    universes.dedup();
    let items: Vec<_> = catalogue()
        .iter()
        .filter(|e| {
            matches(e, &query(), &lang)
                && (universe().is_empty() || e.tool.universe == universe())
                && (!only_installed()
                    || installed_items
                        .read()
                        .iter()
                        .any(|(id, _)| id == &e.tool.id))
        })
        .collect();
    let detail = selected().and_then(|id| kortex_shell::find(&id));
    rsx! {
        style { {include_str!("market.css")} }
        Frame { title:"Kortex Store", subtitle:if fr{"Vos outils. Votre appareil. Vos données."}else{"Your tools. Your device. Your data."},
            section { class:"store-hero k-card",
                p { class:"app-eyebrow", if fr{"38 OUTILS · GRATUITS · LOCAUX"}else{"38 TOOLS · FREE · LOCAL"} }
                h2 { if fr{"Tout ce qu’il vous faut, à portée de main."}else{"Everything you need, within reach."} }
                p {class:"k-muted",if fr{"Explorez les outils, vérifiez leurs permissions et ouvrez-les sans envoyer vos fichiers."}else{"Explore tools, review their permissions and open them without uploading your files."}}
                a {class:"k-btn k-btn-ghost",href:format!("{}{}",url("root",&lang),if fr{"compte"}else{"account"}),if fr{"Gérer mon compte et mes appareils"}else{"Manage my account and devices"}}
            }
            section {class:"store-delivery k-card","aria-label":"Distribution",
                h2 {if fr{"Installation sur ordinateur"}else{"Desktop installation"}}
                p {class:"k-muted",if fr{"Les paquets Windows, macOS et Linux sont en préparation. Les téléchargements et les installations seront ouverts après vérification des signatures et du stockage sécurisé des jetons."}else{"Windows, macOS and Linux packages are being prepared. Downloads and installation will open after signature and secure token storage checks."}}
                if !is_web {
                    p {if fr{"Installations locales détectées : "}else{"Local installations detected: "},"{installed_items.read().len()}"}
                    Button {ghost:true,onclick:move |_|installed_items.set(installed()),if fr{"Actualiser cet appareil"}else{"Refresh this device"}}
                }
            }
            section { class:"store-catalogue", "aria-label":"Catalogue",
                div {class:"store-filters",
                    input {class:"k-input",r#type:"search",value:query(),placeholder:if fr{"Rechercher parmi les 38 outils"}else{"Search all 38 tools"},"aria-label":if fr{"Rechercher un outil"}else{"Search tools"},oninput:move |e|query.set(e.value())}
                    select {class:"k-input",value:universe(),"aria-label":if fr{"Univers"}else{"Universe"},onchange:move |e|universe.set(e.value()),
                        option {value:"",if fr{"Tous les univers"}else{"All universes"}}
                        for u in universes {option {value:u.clone(),"{u}"}}
                    }
                    if !is_web {
                        label {input {r#type:"checkbox",checked:only_installed(),onchange:move |e|only_installed.set(e.checked())}if fr{"Installés sur cet appareil"}else{"Installed on this device"}}
                    }
                }
                p {class:"k-muted",role:"status","{items.len()} ",if fr{"outil(s)"}else{"tool(s)"}}
                if items.is_empty() {p {if fr{"Aucun outil ne correspond. Essayez un autre mot ou univers."}else{"No matching tools. Try another word or universe."}}}
                div {class:"store-grid",
                    for entry in items {
                        article {class:"k-card store-tool",key:"{entry.tool.id}","data-tool":entry.tool.id.clone(),
                            span {class:"app-eyebrow","{entry.tool.universe}"}
                            h2 {"{name(entry,&lang)}"}
                            p {class:"k-muted","{entry.tool.tagline.get(&lang).cloned().unwrap_or_default()}"}
                            div {class:"store-badges",span {if fr{"Gratuit"}else{"Free"}}span {"Rust"}span {"v{entry.tool.version}"}}
                            if let Some((_, version))=installed_items.read().iter().find(|(id,_)|id==&entry.tool.id) {
                                p {if fr{"Installé sur cet appareil : "}else{"Installed on this device: "},"{version}"}
                            }
                            Button {ghost:true,onclick:{let id=entry.tool.id.clone();move |_|{selected.set(Some(id.clone()));launch_failed.set(false);}},if fr{"Voir la fiche"}else{"View details"}}
                            a {class:"k-btn",href:format!("{}{}?open=1",url("root",&lang),entry.tool.slug.get(&lang).cloned().unwrap_or_default()),if fr{"Ouvrir sur le web"}else{"Open on the web"}}
                        }
                    }
                }
            }
            if let Some(entry)=detail {
                section {class:"k-card store-detail",role:"region","aria-label":if fr{"Fiche de l’outil"}else{"Tool details"},"data-detail":entry.tool.id.clone(),
                    Button {ghost:true,onclick:move |_|selected.set(None),if fr{"Fermer la fiche"}else{"Close details"}}
                    h2 {"{name(entry,&lang)}"}
                    p {"{entry.tool.tagline.get(&lang).cloned().unwrap_or_default()}"}
                    dl {
                        dt {if fr{"Identifiant"}else{"Identifier"}}dd {"{entry.tool.id}"}
                        dt {"Version"}dd {"{entry.tool.version}"}
                        dt {if fr{"Traitement"}else{"Processing"}}dd {if fr{"Local sur votre appareil"}else{"Locally on your device"}}
                        dt {if fr{"Hors ligne"}else{"Offline"}}dd {if entry.tool.offline {if fr{"Oui, après chargement"}else{"Yes, after loading"}}else {if fr{"Non déclaré"}else{"Not declared"}}}
                    }
                    if let Some(permission) = permissions(&entry.tool.id) {
                        h3 {if fr{"Permissions déclarées"}else{"Declared permissions"}}
                        dl {
                            dt {if fr{"Réseau"}else{"Network"}}dd {if permission.network {if fr{"Oui"}else{"Yes"}}else {if fr{"Non"}else{"No"}}}
                            dt {if fr{"Fichiers"}else{"Files"}}dd {"{permission.filesystem}"}
                            dt {if fr{"Presse-papiers"}else{"Clipboard"}}dd {"{permission.clipboard}"}
                            dt {if fr{"Confirmation sensible"}else{"Sensitive confirmation"}}dd {if permission.sensitive {if fr{"Requise"}else{"Required"}}else {if fr{"Non déclarée"}else{"Not declared"}}}
                        }
                    }
                    a {class:"k-btn",href:format!("{}{}?open=1",url("root",&lang),entry.tool.slug.get(&lang).cloned().unwrap_or_default()),if fr{"Utiliser cet outil"}else{"Use this tool"}}
                    if !is_web && installed_items.read().iter().any(|(id,_)|id==&entry.tool.id) {
                        Button {onclick:{let id=entry.tool.id.clone();move |_|launch_failed.set(!kortex_shell::launch_installed(&id))},if fr{"Ouvrir l’outil installé"}else{"Open installed tool"}}
                    }
                    if launch_failed() {p {role:"alert",if fr{"Ouverture impossible. Vérifiez l’installation."}else{"Unable to open. Check the installation."}}}
                }
            }
        }
    }
}
